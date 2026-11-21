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
    use super::{MOST, Reported, Unkept, figures, of_web, page, record};

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
