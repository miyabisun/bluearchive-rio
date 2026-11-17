//! The rows of the game's list of battle records as Rio keeps them: telling a row seen again
//! from a new one. A row tells who fought as what and who won, and its time only roughly
//! ("2時間前"); the same two players may fight twice within a minute, so two rows may tell
//! the same. What tells them apart is where they stand: the list runs from the newest
//! battle down, and the rows in sight stand side by side in it.

use std::time::{Duration, Instant};

/// What a row tells of a battle that stays as it is: whether the user attacked and won,
/// and the opponent (the player's number; `None` for an anonymous one).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fight {
    pub attacked: bool,
    pub won: bool,
    pub opponent: Option<i64>,
}

/// A row seen in a frame: the fight, and the earliest and the latest its time allows
/// (seconds since 1970, see `list::Ago::range`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sighting {
    pub fight: Fight,
    pub range: (i64, i64),
}

/// A row kept: where it stands in the list (a larger `place` is a newer battle), and the
/// record read of its battle, once its record was opened.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Listed {
    pub id: i64,
    pub place: f64,
    pub fight: Fight,
    pub range: (i64, i64),
    pub battle: Option<i64>,
}

/// Which row kept a row seen is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Match {
    /// The row kept at this index.
    Kept(usize),
    /// A row not kept yet.
    New,
    /// Not known: the row did not read, or it may be either of two rows kept.
    Unsure,
}

/// Two ranges of one battle's time may miss each other by this much: a row read a moment
/// before the minute turns.
const SLACK: i64 = 60;

/// Which of the rows kept (`kept`, the newest first) each row in sight is (`seen`, from the
/// top; `None` for a row that did not read).
pub fn align(kept: &[Listed], seen: &[Option<Sighting>]) -> Vec<Match> {
    // Where two rows kept fit a row equally well, it is told only if it is the same row
    // whichever of them is preferred.
    let (upper, lower) = (fitted(kept, seen, false), fitted(kept, seen, true));
    seen.iter()
        .enumerate()
        .map(|(at, row)| match (row, upper[at], lower[at]) {
            (Some(_), one, other) if one == other => one.map_or(Match::New, Match::Kept),
            _ => Match::Unsure,
        })
        .collect()
}

/// How well the rows in sight fit the rows kept: the rows told, then the rows kept that
/// are passed over between two told (but for a row in sight that did not read), then the
/// rows in sight left new between two told. The rows in sight stand side by side in the
/// list, so the rows kept that they are stand side by side too, and a new row stands
/// rather above or under them than among them.
type Fit = (i32, i32, i32);
/// A row in sight told as a row kept: the two's indices.
type Pair = (usize, usize);
/// The best fit that ends with a pair, and the pair it follows.
type Ending = (Fit, Option<Pair>);
/// What a fit is preferred by: how good it is, then where its last pair lies.
type Rank = (Fit, i64, usize);

/// The row kept each row in sight is, of the best fit: of two fits as good, the one that
/// ends at the row kept further down if `lower`, and at the row in sight further down.
fn fitted(kept: &[Listed], seen: &[Option<Sighting>], lower: bool) -> Vec<Option<usize>> {
    let same = |row: &Sighting, kept: &Listed| {
        row.fight == kept.fight
            && row.range.0 <= kept.range.1 + SLACK
            && kept.range.0 <= row.range.1 + SLACK
    };
    let prefer = |at: usize| if lower { at as i64 } else { -(at as i64) };
    // The best fit that ends with the row in sight `row` told as the row kept `at`.
    let mut fits: Vec<Vec<Option<Ending>>> = vec![vec![None; kept.len()]; seen.len()];
    let mut best: Option<(Rank, Pair)> = None;
    for row in 0..seen.len() {
        let Some(sighting) = &seen[row] else {
            continue;
        };
        for at in 0..kept.len() {
            if !same(sighting, &kept[at]) {
                continue;
            }
            let mut fit: Ending = ((1, 0, 0), None);
            let mut rank: Rank = ((1, 0, 0), i64::MIN, 0);
            for before in 0..row {
                let unread = seen[before + 1..row].iter().filter(|row| row.is_none());
                let unread = unread.count() as i32;
                let new = (row - before - 1) as i32 - unread;
                for (above, ending) in fits[before].iter().enumerate().take(at) {
                    let Some(((told, passed, among), _)) = ending else {
                        continue;
                    };
                    let over = ((at - above - 1) as i32 - unread).max(0);
                    let next = (told + 1, passed - over, among - new);
                    if (next, prefer(above), before) > rank {
                        rank = (next, prefer(above), before);
                        fit = (next, Some((before, above)));
                    }
                }
            }
            fits[row][at] = Some(fit);
            let rank = (fit.0, prefer(at), row);
            if best.is_none_or(|(known, _)| rank > known) {
                best = Some((rank, (row, at)));
            }
        }
    }
    let mut told = vec![None; seen.len()];
    let mut pair = best.map(|(_, pair)| pair);
    while let Some((row, at)) = pair {
        told[row] = Some(at);
        pair = fits[row][at].and_then(|(_, before)| before);
    }
    told
}

/// The range of a battle's time kept so far, narrowed by what a row tells of it now.
pub fn narrowed(kept: (i64, i64), seen: (i64, i64)) -> (i64, i64) {
    let (earliest, latest) = (kept.0.max(seen.0), kept.1.min(seen.1));
    if earliest <= latest {
        (earliest, latest)
    } else {
        kept
    }
}

/// Where the new rows among those in sight stand: a place for each `Match::New`, between
/// the places of the rows kept around it; none for a new row whose place nothing tells.
pub fn places(kept: &[Listed], seen: &[Option<Sighting>], matches: &[Match]) -> Vec<Option<f64>> {
    let mut places = vec![None; seen.len()];
    // The new rows from `from` up to `to`, spread between the places `above` and `under`.
    let mut spread = |from: usize, to: usize, above: Option<f64>, under: Option<f64>| {
        let new: Vec<usize> = (from..to).filter(|at| matches[*at] == Match::New).collect();
        let count = new.len() as f64;
        for (nth, at) in new.into_iter().enumerate() {
            let nth = nth as f64 + 1.0;
            places[at] = Some(match (above, under) {
                (Some(above), Some(under)) => above - (above - under) * nth / (count + 1.0),
                (None, Some(under)) => under + count - nth + 1.0,
                (Some(above), None) => above - nth,
                (None, None) => count - nth + 1.0,
            });
        }
    };
    let place = |at: usize| kept.get(at).map(|kept| kept.place);
    let told: Vec<(usize, usize)> = matches
        .iter()
        .enumerate()
        .filter_map(|(row, told)| match told {
            Match::Kept(at) => Some((row, *at)),
            _ => None,
        })
        .collect();
    let (Some(&(first, top)), Some(&(last, bottom))) = (told.first(), told.last()) else {
        // No row kept in sight: the rows stand where their times put them, if every row
        // kept is surely newer than all of them or surely older. Among rows kept that may
        // be as old (the rows of one day tell the same time), nothing in sight tells
        // where they stand: they are left unplaced, to be kept once the list is scrolled
        // over them and a row beside them.
        let read = || seen.iter().flatten().map(|row| row.range);
        let (Some(earliest), Some(latest)) = (
            read().map(|range| range.0).min(),
            read().map(|range| range.1).max(),
        ) else {
            return places;
        };
        let newer = kept.iter().take_while(|kept| kept.range.0 > latest).count();
        if kept[newer..].iter().all(|kept| kept.range.1 < earliest) {
            let above = newer.checked_sub(1).and_then(place);
            spread(0, seen.len(), above, place(newer));
        }
        return places;
    };
    spread(0, first, top.checked_sub(1).and_then(place), place(top));
    for pair in told.windows(2) {
        spread(pair[0].0, pair[1].0, place(pair[0].1), place(pair[1].1));
    }
    spread(last, seen.len(), place(bottom), place(bottom + 1));
    places
}

/// What a frame of the game shows, for telling how long the list has been up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shows {
    List,
    /// A battle record: opened from the list, which stays up behind it.
    Record,
    Other,
}

/// How long the list of records has been up. Its rows tell the time as it was when the
/// list came up ("5分前" stays "5分前" for as long as the list is left open: seen on
/// 2026-11-18 for a quarter of an hour), so what a row tells counts from then. A list that
/// was up before Rio looked has no such time.
#[derive(Debug, Default)]
pub struct Up {
    /// When the list came up (seconds since 1970) where Rio saw it come, and when the list
    /// or a record over it was last seen.
    up: Option<(Option<i64>, Instant)>,
    /// When another screen was last seen.
    other: Option<Instant>,
}

/// The list is gone when neither it nor a record was seen for this long: a record opens
/// and closes over it in less.
const GONE: Duration = Duration::from_secs(2);
/// The list came up under Rio's eyes if another screen was seen this shortly before it.
const JUST: Duration = Duration::from_millis(1500);

impl Up {
    /// What a frame read at `now` shows, `clock` seconds since 1970. For the list or a
    /// record: when the list came up, `None` if Rio did not see it come.
    pub fn see(&mut self, shows: Shows, now: Instant, clock: i64) -> Option<i64> {
        let gone = |seen: Instant| now.duration_since(seen) > GONE;
        if self.up.is_some_and(|(_, seen)| gone(seen)) {
            self.up = None;
        }
        if shows == Shows::Other {
            self.other = Some(now);
            return None;
        }
        let since = match self.up {
            Some((since, _)) => since,
            None => {
                let just = |other: Instant| now.duration_since(other) <= JUST;
                (shows == Shows::List && self.other.is_some_and(just)).then_some(clock)
            }
        };
        self.up = Some((since, now));
        since
    }
}

/// What the pointer is over as the list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pointed {
    /// The row in full sight at this index.
    Row(usize),
    /// The game, but none of the rows in full sight.
    Elsewhere,
    /// Not known: the pointer is not over the game.
    #[default]
    Unknown,
}

/// What the pointer is over: `at` on the measured screen (`None` off the game), the rows in
/// full sight with their tops at `tops`, each `tall` high and reaching `across`.
pub fn pointed(at: Option<(f32, f32)>, tops: &[f32], tall: f32, across: (f32, f32)) -> Pointed {
    let Some((x, y)) = at else {
        return Pointed::Unknown;
    };
    let over = |top: &f32| (across.0..across.1).contains(&x) && (*top..top + tall).contains(&y);
    tops.iter()
        .position(over)
        .map_or(Pointed::Elsewhere, Pointed::Row)
}

/// A row of the list as it was told when the list was last seen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Told {
    pub id: i64,
    pub fight: Fight,
    pub battle: Option<i64>,
}

/// The row a record was opened from: the rows in full sight when the list was last seen
/// (`None` for a row not told), what the pointer was over then, the fight the record shows
/// and the battle's number. `None` when no row is surely the record's: a record may be
/// opened from a row cut by the list's edge, which is not read.
pub fn opened(rows: &[Option<Told>], pointed: Pointed, fight: Fight, battle: i64) -> Option<i64> {
    let same = |row: &&Told| row.fight == fight;
    match pointed {
        Pointed::Row(at) => rows.get(at)?.as_ref().filter(same).map(|row| row.id),
        Pointed::Elsewhere => None,
        Pointed::Unknown => {
            let like: Vec<&Told> = rows.iter().flatten().filter(same).collect();
            let own = like.iter().find(|row| row.battle == Some(battle));
            let free = like.iter().find(|row| row.battle.is_none());
            match like[..] {
                [only] => Some(only.id),
                _ => own.or(free).map(|row| row.id),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{
        Fight, Listed, Match, Pointed, Shows, Sighting, Told, Up, align, narrowed, opened, places,
        pointed,
    };

    /// A fight against the player `opponent`: a defense won.
    fn fight(opponent: i64) -> Fight {
        Fight {
            attacked: false,
            won: true,
            opponent: Some(opponent),
        }
    }

    /// A row that tells a minute ending at `at`.
    fn seen(opponent: i64, at: i64) -> Option<Sighting> {
        Some(Sighting {
            fight: fight(opponent),
            range: (at - 60, at),
        })
    }

    /// The rows kept, the newest first: each an opponent and the end of its minute.
    fn kept(rows: &[(i64, i64)]) -> Vec<Listed> {
        rows.iter()
            .enumerate()
            .map(|(at, (opponent, time))| Listed {
                id: 100 + at as i64,
                place: (rows.len() - at) as f64,
                fight: fight(*opponent),
                range: (time - 60, *time),
                battle: None,
            })
            .collect()
    }

    use Match::{Kept, New, Unsure};

    #[test]
    fn rows_never_seen_are_new() {
        assert_eq!(align(&[], &[seen(1, 9000), seen(2, 8000)]), [New, New]);
    }

    #[test]
    fn rows_seen_again_are_the_rows_kept() {
        let kept = kept(&[(1, 9000), (2, 8000), (3, 7000)]);
        assert_eq!(
            align(&kept, &[seen(1, 9000), seen(2, 8000), seen(3, 7000)]),
            [Kept(0), Kept(1), Kept(2)]
        );
        // The list scrolled: the rows further down.
        assert_eq!(
            align(&kept, &[seen(2, 8000), seen(3, 7000)]),
            [Kept(1), Kept(2)]
        );
        // A battle since: one more row on top.
        assert_eq!(
            align(&kept, &[seen(4, 9900), seen(1, 9000), seen(2, 8000)]),
            [New, Kept(0), Kept(1)]
        );
    }

    #[test]
    fn a_row_is_told_by_its_fight_and_its_time() {
        let kept = kept(&[(1, 9000)]);
        assert_eq!(align(&kept, &[seen(2, 9000)]), [New], "another opponent");
        assert_eq!(align(&kept, &[seen(1, 5000)]), [New], "another time");
        assert_eq!(
            align(&kept, &[seen(1, 9100)]),
            [Kept(0)],
            "a minute that turned as the row was read"
        );
        let lost = Some(Sighting {
            fight: Fight {
                won: false,
                ..fight(1)
            },
            range: (8940, 9000),
        });
        assert_eq!(align(&kept, &[lost]), [New], "another winner");
    }

    #[test]
    fn two_rows_that_tell_the_same_are_told_by_the_rows_beside_them() {
        // The same opponent beaten twice within the minute, then another.
        let twins = kept(&[(1, 9000), (1, 9000), (2, 8000)]);
        assert_eq!(
            align(&twins, &[seen(1, 9000), seen(2, 8000)]),
            [Kept(1), Kept(2)],
            "the one just above the row below"
        );
        let twins = kept(&[(2, 9900), (1, 9000), (1, 9000)]);
        assert_eq!(
            align(&twins, &[seen(2, 9900), seen(1, 9000)]),
            [Kept(0), Kept(1)],
            "the one just under the row above"
        );
        assert_eq!(
            align(&twins, &[seen(1, 9000), seen(1, 9000)]),
            [Kept(1), Kept(2)],
            "both in sight"
        );
        assert_eq!(
            align(&twins, &[seen(1, 9000)]),
            [Unsure],
            "alone in sight, it may be either"
        );
    }

    #[test]
    fn a_new_row_like_one_kept_stands_above_it() {
        // The opponent beaten again: the row kept is the older of the two.
        let kept = kept(&[(1, 9000), (2, 8000)]);
        assert_eq!(
            align(&kept, &[seen(1, 9000), seen(1, 9000), seen(2, 8000)]),
            [New, Kept(0), Kept(1)]
        );
        // Without a row beside them to tell by, the newer battle is the new row still.
        assert_eq!(
            align(&kept[..1], &[seen(1, 9000), seen(1, 9000)]),
            [New, Kept(0)]
        );
    }

    #[test]
    fn a_row_that_did_not_read_is_not_told() {
        let kept = kept(&[(1, 9000), (2, 8000), (3, 7000)]);
        assert_eq!(
            align(&kept, &[seen(1, 9000), None, seen(3, 7000)]),
            [Kept(0), Unsure, Kept(2)]
        );
    }

    #[test]
    fn a_time_told_again_narrows_the_range() {
        assert_eq!(narrowed((1000, 4600), (2000, 5600)), (2000, 4600));
        assert_eq!(narrowed((1000, 4600), (0, 9000)), (1000, 4600));
        assert_eq!(
            narrowed((1000, 2000), (2030, 3000)),
            (1000, 2000),
            "ranges that miss each other narrow nothing"
        );
    }

    #[test]
    fn a_new_row_stands_between_the_rows_around_it() {
        let rows = kept(&[(1, 9000), (2, 8000)]);
        let place = |seen: &[Option<Sighting>]| places(&rows, seen, &align(&rows, seen));
        // Places 2 and 1 are kept: a row on top stands above 2.
        assert_eq!(place(&[seen(3, 9900), seen(1, 9000)]), [Some(3.0), None]);
        assert_eq!(
            place(&[seen(4, 9950), seen(3, 9900), seen(1, 9000)]),
            [Some(4.0), Some(3.0), None],
            "two on top, the upper the newer"
        );
        assert_eq!(
            place(&[seen(1, 9000), seen(3, 8500), seen(2, 8000)]),
            [None, Some(1.5), None],
            "a row between two kept"
        );
        assert_eq!(
            place(&[seen(2, 8000), seen(3, 7000), seen(4, 6000)]),
            [None, Some(0.0), Some(-1.0)],
            "rows under the last kept"
        );
        // With no row kept in sight, the rows stand where their time puts them.
        assert_eq!(place(&[seen(3, 9900)]), [Some(3.0)], "newer than all");
        assert_eq!(place(&[seen(3, 8500)]), [Some(1.5)], "between");
        assert_eq!(place(&[seen(3, 7000)]), [Some(0.0)], "older than all");
        assert_eq!(
            places(
                &[],
                &[seen(1, 9000), seen(2, 8000)],
                &[Match::New, Match::New]
            ),
            [Some(2.0), Some(1.0)],
            "the first rows ever"
        );
    }

    #[test]
    fn a_new_row_is_not_placed_where_nothing_tells_its_place() {
        // A row kept tells a day; the list scrolled far shows a row of the same day, and no
        // row kept beside it: it may stand above the row kept or under it.
        let day = |opponent, at: i64| {
            Some(Sighting {
                fight: fight(opponent),
                range: (at - 86400, at),
            })
        };
        let rows = vec![Listed {
            id: 100,
            place: 1.0,
            fight: fight(1),
            range: (500_000 - 86400, 500_000),
            battle: None,
        }];
        let seen = [day(2, 500_030), day(3, 500_030)];
        let matches = align(&rows, &seen);
        assert_eq!(matches, [New, New]);
        assert_eq!(places(&rows, &seen, &matches), [None, None]);
        // Seen with the row kept beside them, they stand under it.
        let seen = [day(1, 500_030), day(2, 500_030), day(3, 500_030)];
        let matches = align(&rows, &seen);
        assert_eq!(
            places(&rows, &seen, &matches),
            [None, Some(0.0), Some(-1.0)]
        );
        // Rows a day older than all kept stand under them, whatever is in sight.
        let seen = [day(2, 400_000)];
        assert_eq!(places(&rows, &seen, &align(&rows, &seen)), [Some(0.0)]);
    }

    #[test]
    fn a_new_row_under_a_kept_one_stands_above_the_next_kept() {
        // Places 3, 2, 1 kept; the list shows the first and a row never read under it.
        let rows = kept(&[(1, 9000), (2, 8000), (3, 7000)]);
        let seen = [seen(1, 9000), seen(4, 8500)];
        assert_eq!(
            places(&rows, &seen, &align(&rows, &seen)),
            [None, Some(2.5)]
        );
    }

    /// What the frames from `from` to `to` (milliseconds after `start`, one every half second)
    /// tell when they all show the same: the last of them. The clock runs with them, from
    /// 1000 seconds.
    fn frames(up: &mut Up, shows: Shows, start: Instant, from: u64, to: u64) -> Option<i64> {
        let mut told = None;
        for millis in (from..=to).step_by(500) {
            let clock = 1000 + (millis / 1000) as i64;
            told = up.see(shows, start + Duration::from_millis(millis), clock);
        }
        told
    }

    #[test]
    fn the_list_is_up_since_it_came_under_rios_eyes() {
        let start = Instant::now();
        let mut up = Up::default();
        frames(&mut up, Shows::Other, start, 0, 0);
        assert_eq!(
            frames(&mut up, Shows::List, start, 100, 100),
            Some(1000),
            "seen to come up"
        );
        assert_eq!(
            frames(&mut up, Shows::List, start, 600, 60_000),
            Some(1000),
            "left open"
        );
        // A record opened over it, read for a while, and closed again.
        frames(&mut up, Shows::Other, start, 60_100, 60_100);
        assert_eq!(
            frames(&mut up, Shows::Record, start, 60_400, 90_000),
            Some(1000)
        );
        frames(&mut up, Shows::Other, start, 90_100, 90_100);
        assert_eq!(
            frames(&mut up, Shows::List, start, 90_500, 90_500),
            Some(1000),
            "the same list"
        );
        // Closed, and opened again later.
        frames(&mut up, Shows::Other, start, 91_000, 95_000);
        assert_eq!(
            frames(&mut up, Shows::List, start, 95_100, 95_100),
            Some(1095),
            "another list"
        );
    }

    #[test]
    fn a_list_up_before_rio_looked_has_no_time() {
        let start = Instant::now();
        let mut up = Up::default();
        assert_eq!(
            frames(&mut up, Shows::List, start, 0, 5000),
            None,
            "from the first frame, and while it stays"
        );
        // Rio paused for a minute over the list: it may have been opened anew meanwhile.
        let mut up = Up::default();
        frames(&mut up, Shows::Other, start, 0, 0);
        assert_eq!(frames(&mut up, Shows::List, start, 100, 100), Some(1000));
        assert_eq!(frames(&mut up, Shows::List, start, 60_000, 60_000), None);
        // A record already open as Rio looked, and the list behind it.
        let mut up = Up::default();
        frames(&mut up, Shows::Other, start, 0, 0);
        assert_eq!(frames(&mut up, Shows::Record, start, 100, 5000), None);
        frames(&mut up, Shows::Other, start, 5100, 5100);
        assert_eq!(frames(&mut up, Shows::List, start, 5400, 5400), None);
    }

    #[test]
    fn the_pointer_is_over_a_row_or_not() {
        let tops = [398.0, 573.0, 747.0];
        let over = |at| pointed(at, &tops, 165.0, (463.0, 1907.0));
        assert_eq!(over(Some((1750.0, 480.0))), Pointed::Row(0), "a button");
        assert_eq!(over(Some((600.0, 700.0))), Pointed::Row(1));
        assert_eq!(over(Some((1750.0, 980.0))), Pointed::Elsewhere, "a row cut");
        assert_eq!(
            over(Some((1750.0, 568.0))),
            Pointed::Elsewhere,
            "between two"
        );
        assert_eq!(
            over(Some((300.0, 480.0))),
            Pointed::Elsewhere,
            "beside the list"
        );
        assert_eq!(over(None), Pointed::Unknown, "off the game");
    }

    fn told(id: i64, opponent: i64, battle: Option<i64>) -> Option<Told> {
        Some(Told {
            id,
            fight: fight(opponent),
            battle,
        })
    }

    #[test]
    fn a_record_is_the_row_it_was_opened_from() {
        let rows = [told(11, 1, None), told(12, 2, None), None];
        assert_eq!(opened(&rows, Pointed::Row(1), fight(2), 7), Some(12));
        assert_eq!(
            opened(&rows, Pointed::Unknown, fight(2), 7),
            Some(12),
            "the one row that tells the same"
        );
        assert_eq!(
            opened(&rows, Pointed::Row(0), fight(2), 7),
            None,
            "the pointer's row tells another fight"
        );
        assert_eq!(
            opened(&rows, Pointed::Row(2), fight(2), 7),
            None,
            "the pointer's row was not told"
        );
        assert_eq!(
            opened(&rows, Pointed::Elsewhere, fight(2), 7),
            None,
            "opened from a row cut by the list's edge"
        );
        assert_eq!(
            opened(&rows, Pointed::Unknown, fight(3), 7),
            None,
            "no such row"
        );
    }

    #[test]
    fn of_rows_that_tell_the_same_the_pointer_tells_the_one() {
        let twins = [told(11, 1, None), told(12, 1, None)];
        assert_eq!(opened(&twins, Pointed::Row(1), fight(1), 7), Some(12));
        // Without the pointer: the record's own row if it has one, or else the first row
        // without a record, so that both have theirs once both were opened.
        assert_eq!(opened(&twins, Pointed::Unknown, fight(1), 7), Some(11));
        let one_read = [told(11, 1, Some(7)), told(12, 1, None)];
        assert_eq!(opened(&one_read, Pointed::Unknown, fight(1), 7), Some(11));
        assert_eq!(opened(&one_read, Pointed::Unknown, fight(1), 8), Some(12));
        let both_read = [told(11, 1, Some(7)), told(12, 1, Some(8))];
        assert_eq!(opened(&both_read, Pointed::Unknown, fight(1), 9), None);
        // The pointer says the record is the other row's after all.
        assert_eq!(opened(&one_read, Pointed::Row(1), fight(1), 7), Some(12));
    }
}
