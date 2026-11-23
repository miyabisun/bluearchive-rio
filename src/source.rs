//! What a formation registered by hand says beside its twelve students: the record it was
//! introduced with ("20戦18勝", as the post or the video that told of it shows) and the page
//! it comes from. Either may be missing. What the user types of them in the window, and what
//! a client of the MCP server sends, is taken through here: nothing else is kept.

/// A record a formation was introduced with: battles of whoever told of it, not the user's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reported {
    pub battles: u32,
    pub wins: u32,
}

/// The most battles a record tells of, and the figures of that number: what a field takes.
pub const MOST: u32 = 9999;
const FIGURES: usize = 4;
/// The longest address kept.
const LONGEST: usize = 2000;

impl Reported {
    /// A record of `wins` in `battles`: a battle at least and `MOST` at most, and no more
    /// wins than battles. Anything else is no record.
    pub fn new(battles: u32, wins: u32) -> Option<Self> {
        let told = (1..=MOST).contains(&battles) && wins <= battles;
        told.then_some(Self { battles, wins })
    }
}

/// What a registration says beside its students.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Source {
    pub reported: Option<Reported>,
    /// The page the formation comes from, an address of the web.
    pub url: Option<String>,
}

/// Why what was typed is not kept: it tells no record, or no address of the web.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unkept;

/// What a field of the record holds of what was typed into it: the digits (those of full
/// width too, as the IME gives them), `FIGURES` at most. Nothing else can be entered.
pub fn figures(typed: &str) -> String {
    let digit = |c: char| match c {
        '0'..='9' => Some(c),
        // '０' to '９' are their digits more 0xfee0.
        '０'..='９' => char::from_u32(u32::from(c) - 0xfee0),
        _ => None,
    };
    typed.chars().filter_map(digit).take(FIGURES).collect()
}

/// The record the two fields tell: none while both are empty; `Unkept` while they tell no
/// record (one of them empty, no battle, more wins than battles).
pub fn record(battles: &str, wins: &str) -> Result<Option<Reported>, Unkept> {
    let (battles, wins) = (battles.trim(), wins.trim());
    if battles.is_empty() && wins.is_empty() {
        return Ok(None);
    }
    let figure = |typed: &str| typed.parse::<u32>().map_err(|_| Unkept);
    let told = Reported::new(figure(battles)?, figure(wins)?);
    told.map(Some).ok_or(Unkept)
}

/// A field of the record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Figure {
    Battles,
    Wins,
}

/// What the two fields hold after a step in one of them, `up` or down (the arrow keys): a
/// battle more or less, a battle at least and `MOST` at most; a win more or less, none at
/// least and `MOST` at most. A field that tells no number holds one after a step in it.
/// The other field is left as it is, but for more wins than battles: battles stepped down
/// take the wins down with them, wins stepped up take the battles up with them (a battle
/// more, won), and wins stepped down stay within the battles.
pub fn stepped(battles: &str, wins: &str, of: Figure, up: bool) -> (String, String) {
    let number = |typed: &str| typed.trim().parse::<u32>().ok();
    let step = |from: Option<u32>| match from {
        Some(from) if up => from.saturating_add(1),
        Some(from) => from.saturating_sub(1),
        None => 1,
    };
    let (fought, won) = (number(battles), number(wins));
    match of {
        Figure::Battles => {
            let fought = step(fought).clamp(1, MOST);
            let wins = match won {
                Some(won) if won > fought => fought.to_string(),
                _ => wins.to_owned(),
            };
            (fought.to_string(), wins)
        }
        Figure::Wins => {
            let won = step(won).min(MOST);
            match fought {
                Some(fought) if won > fought && up => (won.to_string(), won.to_string()),
                Some(fought) => (battles.to_owned(), won.min(fought).to_string()),
                None => (battles.to_owned(), won.to_string()),
            }
        }
    }
}

/// The page a field tells, without the spaces around it: none while it is empty; `Unkept`
/// for what is no address of the web.
pub fn page(typed: &str) -> Result<Option<String>, Unkept> {
    let typed = typed.trim();
    if typed.is_empty() {
        return Ok(None);
    }
    if of_web(typed) {
        Ok(Some(typed.to_owned()))
    } else {
        Err(Unkept)
    }
}

/// Whether an address is of the web (http or https, with a host, in one piece): the only
/// ones kept, and the only ones the window opens.
pub fn of_web(url: &str) -> bool {
    let small = url.to_ascii_lowercase();
    let after = (small.strip_prefix("https://")).or_else(|| small.strip_prefix("http://"));
    let Some(after) = after else {
        return false;
    };
    // The host ends at the path, the query or the fragment.
    let host = after.split(['/', '?', '#']).next().unwrap_or_default();
    let in_one_piece = !url.chars().any(|c| c.is_whitespace() || c.is_control());
    !host.is_empty() && in_one_piece && url.len() <= LONGEST
}

#[cfg(test)]
mod tests {
    use super::{Figure, MOST, Reported, Unkept, figures, of_web, page, record, stepped};

    #[test]
    fn a_record_has_a_battle_at_least_and_no_more_wins_than_battles() {
        let told =
            |battles, wins| Reported::new(battles, wins).map(|told| (told.battles, told.wins));
        assert_eq!(told(20, 18), Some((20, 18)));
        assert_eq!(told(1, 0), Some((1, 0)), "one battle, lost");
        assert_eq!(told(3, 3), Some((3, 3)), "every battle won");
        assert_eq!(told(3, 4), None, "more wins than battles");
        assert_eq!(told(0, 0), None, "no battle tells nothing");
        assert_eq!(told(MOST, MOST), Some((MOST, MOST)));
        assert_eq!(told(MOST + 1, 0), None, "more than a field takes");
    }

    #[test]
    fn a_field_of_the_record_takes_digits_alone() {
        assert_eq!(figures("20"), "20");
        assert_eq!(
            figures("２０"),
            "20",
            "of full width, as the IME gives them"
        );
        assert_eq!(figures("2a0戦 -"), "20", "nothing else is entered");
        assert_eq!(figures("123456"), "1234", "four figures at most");
        assert_eq!(figures(""), "");
    }

    #[test]
    fn the_fields_tell_a_record_only_when_both_do() {
        assert_eq!(record("", ""), Ok(None), "none is told");
        let told = Reported {
            battles: 20,
            wins: 18,
        };
        assert_eq!(record("20", "18"), Ok(Some(told)));
        assert_eq!(record(" 20 ", "18"), Ok(Some(told)), "spaces aside");
        assert_eq!(record("20", ""), Err(Unkept), "half told");
        assert_eq!(record("", "18"), Err(Unkept));
        assert_eq!(record("18", "20"), Err(Unkept), "more wins than battles");
        assert_eq!(record("0", "0"), Err(Unkept), "no battle");
        assert_eq!(record("二十", "18"), Err(Unkept), "no number");
        assert_eq!(record("-3", "1"), Err(Unkept));
    }

    fn both(battles: &str, wins: &str) -> (String, String) {
        (battles.to_owned(), wins.to_owned())
    }

    #[test]
    fn a_step_in_the_battles_stays_a_record_and_takes_the_wins_along() {
        let (up, down) = (true, false);
        assert_eq!(stepped("1", "1", Figure::Battles, up), both("2", "1"));
        assert_eq!(stepped("20", "18", Figure::Battles, down), both("19", "18"));
        assert_eq!(
            stepped("5", "5", Figure::Battles, down),
            both("4", "4"),
            "no more wins than battles are left"
        );
        assert_eq!(
            stepped("1", "1", Figure::Battles, down),
            both("1", "1"),
            "a battle at least"
        );
        assert_eq!(stepped("1", "0", Figure::Battles, down), both("1", "0"));
        let most = MOST.to_string();
        assert_eq!(
            stepped(&most, "3", Figure::Battles, up),
            both(&most, "3"),
            "as many as a field takes at most"
        );
        // What was typed told no record: a step leaves one.
        assert_eq!(stepped("0", "0", Figure::Battles, down), both("1", "0"));
        assert_eq!(stepped("3", "5", Figure::Battles, up), both("4", "4"));
    }

    #[test]
    fn a_step_in_the_wins_stays_a_record_and_takes_the_battles_up_along() {
        let (up, down) = (true, false);
        assert_eq!(stepped("20", "17", Figure::Wins, up), both("20", "18"));
        assert_eq!(stepped("20", "18", Figure::Wins, down), both("20", "17"));
        assert_eq!(
            stepped("20", "19", Figure::Wins, up),
            both("20", "20"),
            "as many wins as battles yet"
        );
        assert_eq!(
            stepped("20", "20", Figure::Wins, up),
            both("21", "21"),
            "a win more than the battles is a battle more, won"
        );
        assert_eq!(stepped("1", "1", Figure::Wins, up), both("2", "2"));
        let most = MOST.to_string();
        assert_eq!(
            stepped(&most, &most, Figure::Wins, up),
            both(&most, &most),
            "as many as a field takes at most"
        );
        assert_eq!(
            stepped("2", "0", Figure::Wins, down),
            both("2", "0"),
            "no less than none"
        );
        // What was typed told no record: a step leaves one.
        assert_eq!(stepped("3", "5", Figure::Wins, up), both("6", "6"));
        assert_eq!(stepped("3", "5", Figure::Wins, down), both("3", "3"));
        assert_eq!(stepped("0", "0", Figure::Wins, up), both("1", "1"));
    }

    #[test]
    fn a_step_in_an_empty_field_makes_it_one_and_leaves_the_other_empty() {
        for up in [true, false] {
            assert_eq!(stepped("", "", Figure::Battles, up), both("1", ""), "{up}");
            assert_eq!(stepped("", "", Figure::Wins, up), both("", "1"), "{up}");
            assert_eq!(
                stepped("", "3", Figure::Battles, up),
                both("1", "1"),
                "{up}"
            );
            assert_eq!(stepped("20", "", Figure::Wins, up), both("20", "1"), "{up}");
        }
        // The other field stays empty while the one stepped in tells a number.
        assert_eq!(stepped("20", "", Figure::Battles, true), both("21", ""));
        assert_eq!(stepped("20", "", Figure::Battles, false), both("19", ""));
        assert_eq!(stepped("", "3", Figure::Wins, true), both("", "4"));
    }

    #[test]
    fn a_page_is_an_address_of_the_web_or_nothing() {
        assert_eq!(page(""), Ok(None));
        assert_eq!(page("   "), Ok(None), "spaces alone are nothing");
        let post = "https://x.com/someone/status/123?s=20";
        assert_eq!(page(post), Ok(Some(post.to_owned())));
        assert_eq!(
            page(&format!("  {post}\n")),
            Ok(Some(post.to_owned())),
            "pasted with what stood around it"
        );
        assert_eq!(
            page("http://example.com/a"),
            Ok(Some("http://example.com/a".to_owned()))
        );
        for other in [
            "x.com/someone",
            "file:///C:/Windows/notepad.exe",
            "javascript:alert(1)",
            "https://",
            "https:///path",
            "https://x.com/a b",
            "ftp://example.com/a",
        ] {
            assert_eq!(page(other), Err(Unkept), "{other}");
        }
        let long = format!("https://example.com/{}", "a".repeat(2000));
        assert_eq!(page(&long), Err(Unkept), "too long to be one");
    }

    #[test]
    fn only_an_address_of_the_web_is_opened() {
        assert!(of_web("https://www.youtube.com/watch?v=abc"));
        assert!(
            of_web("HTTP://example.com"),
            "however the scheme is written"
        );
        assert!(!of_web("file:///C:/Windows/notepad.exe"));
        assert!(!of_web("notepad.exe"));
        assert!(!of_web("https://example.com/\u{7}"), "a control character");
        assert!(!of_web(""));
    }
}
