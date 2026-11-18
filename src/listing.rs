//! The rows of the game's list of battle records as Rio keeps them: telling a row seen again
//! from a new one. A row tells who fought as what and who won, and its time only roughly
//! ("3日前"): the same two players may fight twice within a minute and again the day after,
//! so many rows tell the same. What tells them apart is the rows beside them. The list runs
//! from the newest battle down and nothing ever comes between two of its rows, so two rows
//! once seen side by side stand side by side for good: Rio keeps for a row the row seen
//! right above it (`Listed::above`), and takes a row in sight for a row kept only where the
//! rows beside it say the same.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

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

/// A row kept: the row seen right above it in the list (`None` where none was seen there
/// yet), and the record read of its battle, once its record was opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Listed {
    pub id: i64,
    pub above: Option<i64>,
    pub fight: Fight,
    pub range: (i64, i64),
    pub battle: Option<i64>,
}

/// Which row kept a row seen is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Match {
    /// The row kept at this index: the rows beside it say so, or it was told so a frame ago.
    Kept(usize),
    /// Likely the row kept at this index: it alone tells so, and nothing beside it tells
    /// either way. Not sure enough to say which rows stand beside it.
    Likely(usize),
    /// A row not kept yet.
    New,
    /// Not known: the row did not read, or it may be either of several rows kept. Whether
    /// its record was read, where all of those say the same.
    Unsure(Option<bool>),
}

/// What a row in sight was told to be: the row kept, where one is told (surely or likely),
/// and whether its record was read, where that is known.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Sight {
    pub row: Option<Told>,
    pub read: Option<bool>,
}

/// What a frame's rows were told to be, and the rows kept that stand right above the first
/// of them and right under the last, where those are surely known: the list's edges cut
/// them, and their buttons may show still.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Sights {
    pub rows: Vec<Sight>,
    pub above: Option<Told>,
    pub below: Option<Told>,
}

/// Two ranges of one battle's time may miss each other by this much: a row read a moment
/// before the minute turns.
const SLACK: i64 = 60;

/// Whether a row seen may be a row kept: the same fight, at a time both allow.
fn like(seen: &Sighting, kept: &Listed) -> bool {
    seen.fight == kept.fight
        && seen.range.0 <= kept.range.1 + SLACK
        && kept.range.0 <= seen.range.1 + SLACK
}

/// Which of the rows kept each row in sight is (`seen`, from the top; `None` for a row that
/// did not read). `carried` tells the rows that were told a frame ago (see `shift`): the
/// numbers of the rows kept they are.
///
/// A row told a frame ago is that row (one that tells another fight now is not told: the
/// list does not change, so its reading is off). From a row told, the row kept beside it is the row
/// in sight beside it, where that tells the same. Of the rest, a row is a row kept if the
/// rows kept beside that one are what the list shows beside it: two rows side by side that
/// are two kept side by side are sure (if they could be no other two), a row with nothing
/// kept beside it to go by is only likely, but for a row kept alone.
pub fn align(kept: &[Listed], seen: &[Option<Sighting>], carried: &[Option<i64>]) -> Vec<Match> {
    let index: HashMap<i64, usize> = kept
        .iter()
        .enumerate()
        .map(|(at, row)| (row.id, at))
        .collect();
    let above: Vec<Option<usize>> = kept
        .iter()
        .map(|row| row.above.and_then(|id| index.get(&id).copied()))
        .collect();
    let mut below = vec![None; kept.len()];
    for (at, above) in above.iter().enumerate() {
        if let Some(above) = above {
            below[*above] = Some(at);
        }
    }
    // The run of rows side by side a row kept stands in (its top row), and how far down.
    let stand = |at: usize| {
        let up = std::iter::successors(Some(at), |at| above[*at]).take(kept.len());
        up.enumerate()
            .last()
            .map_or((at, 0), |(down, top)| (top, down as isize))
    };
    let stands: Vec<(usize, isize)> = (0..kept.len()).map(stand).collect();
    // A row told a frame ago that tells another fight now: its reading is off.
    let off = |(row, id): (&Option<Sighting>, &Option<i64>)| {
        let known = id.and_then(|id| index.get(&id)).map(|at| &kept[*at]);
        row.filter(|row| known.is_none_or(|known| like(row, known)))
    };
    let seen: Vec<Option<Sighting>> = seen.iter().zip(carried).map(off).collect();
    let same = |row: usize, at: usize| seen[row].as_ref().is_some_and(|row| like(row, &kept[at]));
    // The rows in sight above and under a row, and the rows kept above and under one.
    let beside = |row: usize| {
        [
            row.checked_sub(1),
            Some(row + 1).filter(|row| *row < seen.len()),
        ]
    };
    let kept_beside = |at: usize| [above[at], below[at]];
    // Whether the row kept `at` may be the row in sight `row`, going by where it is kept:
    // not in the run of a row told at another place than the list shows it; and along its
    // run up and down, as far as the list shows rows, each row kept is the row in sight
    // there (`backed` if a row that read and is not told yet says so too).
    let fits = |told: &[Option<usize>], row: usize, at: usize| -> Option<bool> {
        // A row told that stands in the same run stands as far from it there as here.
        let apart = |(other, told): (usize, &Option<usize>)| {
            let (run, down) = stands[(*told)?];
            (run == stands[at].0).then_some((down - stands[at].1, other as isize - row as isize))
        };
        let mut apart = told.iter().enumerate().filter_map(apart);
        if apart.any(|(kept, shown)| kept != shown) {
            return None;
        }
        let mut backed = false;
        for (step, next) in [(-1, &above), (1, &below)] {
            let (mut shown, mut known) = (row as isize + step, next[at]);
            let inside = |shown: isize| usize::try_from(shown).ok().filter(|at| *at < seen.len());
            while let (Some(here), Some(at)) = (inside(shown), known) {
                match (told[here], &seen[here]) {
                    (Some(told), _) if told != at => return None,
                    (None, Some(_)) if !same(here, at) || told.contains(&Some(at)) => return None,
                    (Some(_), _) | (None, Some(_)) => backed = true,
                    (None, None) => {}
                }
                (shown, known) = (shown + step, next[at]);
            }
        }
        Some(backed)
    };
    // The rows of each run, and those of them that have a record: by the run's top row.
    let mut runs: HashMap<usize, (usize, usize)> = HashMap::new();
    for (at, (run, _)) in stands.iter().enumerate() {
        let (records, rows) = runs.entry(*run).or_default();
        *records += usize::from(kept[at].battle.is_some());
        *rows += 1;
    }
    // Two rows kept that are one row of the list, kept twice: in two runs, telling the
    // same fight at the same time (half the time the narrower of them allows, at least).
    let twice = |a: usize, b: usize| {
        let (one, other) = (kept[a].range, kept[b].range);
        let shared = one.1.min(other.1) - one.0.max(other.0);
        stands[a].0 != stands[b].0
            && kept[a].fight == kept[b].fight
            && shared * 2 >= (one.1 - one.0).min(other.1 - other.0)
    };
    // Of rows kept that are one row kept twice, the one of the run with more records, of
    // the longer run, kept first; `None` if they are not all one row.
    let one = |fitting: &[usize]| {
        let first = *fitting.first()?;
        let weight = |at: &&usize| (runs[&stands[**at].0], std::cmp::Reverse(**at));
        let all = fitting.iter().all(|at| *at == first || twice(*at, first));
        fitting.iter().max_by_key(weight).filter(|_| all).copied()
    };
    // Tells what the rows kept tell of the rows in sight, from the rows told already.
    let tell = |told: &mut Vec<Option<usize>>| {
        loop {
            let found = {
                let told: &[Option<usize>] = told;
                // From a row told to the rows beside it.
                let reached = (0..seen.len()).find_map(|row| {
                    let at = told[row]?;
                    let next = beside(row).into_iter().zip(kept_beside(at));
                    next.flat_map(|(shown, known)| shown.zip(known))
                        .find(|(shown, known)| {
                            told[*shown].is_none()
                                && same(*shown, *known)
                                && !told.contains(&Some(*known))
                        })
                });
                // A row that only one row kept can be, as the rows along its run say.
                let backed = || {
                    (0..seen.len()).find_map(|row| {
                        let free = |at: &usize| same(row, *at) && !told.contains(&Some(*at));
                        let backed = |at: &usize| fits(told, row, *at) == Some(true);
                        let fitting = (0..kept.len()).filter(free).filter(backed);
                        let fitting: Vec<usize> = fitting.collect();
                        Some((row, one(&fitting).filter(|_| told[row].is_none())?))
                    })
                };
                reached.or_else(backed)
            };
            let Some((row, at)) = found else {
                break;
            };
            told[row] = Some(at);
        }
    };
    // What the rows kept tell by themselves stands, if they tell every row that read: what
    // was carried may be off where rows that tell the same stand side by side. Else, the
    // rows carried are those rows, and the rows kept tell the rest.
    let mut told: Vec<Option<usize>> = vec![None; seen.len()];
    tell(&mut told);
    if told
        .iter()
        .zip(&seen)
        .any(|(told, seen)| told.is_none() && seen.is_some())
    {
        told = vec![None; seen.len()];
        for (row, id) in carried.iter().enumerate().take(seen.len()) {
            let known = id.and_then(|id| index.get(&id).copied());
            if let Some(at) = known.filter(|at| same(row, *at) && !told.contains(&Some(*at))) {
                told[row] = Some(at);
            }
        }
        tell(&mut told);
    }
    // The rest, from the last up: a new battle stands above the older ones like it.
    let mut matches: Vec<Match> = told
        .iter()
        .map(|told| told.map_or(Match::Unsure(None), Match::Kept))
        .collect();
    let mut taken: Vec<usize> = told.iter().flatten().copied().collect();
    let alone = |at: usize| above[at].is_none() && below[at].is_none();
    for row in (0..seen.len()).rev() {
        let (None, Some(sighting)) = (told[row], &seen[row]) else {
            continue;
        };
        let free = |at: &usize| same(row, *at) && !taken.contains(at);
        let mut fitting: Vec<usize> = (0..kept.len())
            .filter(free)
            .filter(|at| fits(&told, row, *at).is_some())
            .collect();
        // Of rows kept alone, nothing but the time tells one from another: the one whose
        // time fits best, the one kept last of those.
        let shared = |at: &usize| {
            let (kept, seen) = (kept[*at].range, sighting.range);
            kept.1.min(seen.1) - kept.0.max(seen.0)
        };
        if fitting.len() > 1 && fitting.iter().all(|at| alone(*at)) {
            fitting = fitting
                .into_iter()
                .max_by_key(|at| (shared(at), *at))
                .into_iter()
                .collect();
        } else if let Some(one) = one(&fitting) {
            fitting = vec![one];
        }
        matches[row] = match fitting[..] {
            [] => Match::New,
            [only] if alone(only) => {
                told[row] = Some(only);
                Match::Kept(only)
            }
            [only] => Match::Likely(only),
            [first, ..] => {
                let read = kept[first].battle.is_some();
                let agreed = fitting.iter().all(|at| kept[*at].battle.is_some() == read);
                Match::Unsure(agreed.then_some(read))
            }
        };
        if let [only] = fitting[..] {
            taken.push(only);
        }
    }
    matches
}

/// The moves of the list between two frames that their rows allow, the least first: `by`
/// where every row of `now` is the row `by` further down in `before` (`same` tells two
/// readings of one row), going by two rows read in both, or by all there are of fewer. None
/// if the frames share too little: the list jumped, or it is another list. Several where
/// the rows both frames show tell the same: such rows do not tell how far the list moved.
pub fn shifts<T>(
    before: &[Option<T>],
    now: &[Option<T>],
    same: impl Fn(&T, &T) -> bool,
) -> Vec<isize> {
    let read = |rows: &[Option<T>]| rows.iter().flatten().count();
    let needed = read(before).min(read(now)).min(2);
    let fits = |by: &isize| {
        let mut alike = 0;
        for (row, now) in now.iter().enumerate() {
            let at = usize::try_from(row as isize + by).ok();
            match (at.and_then(|at| before.get(at)), now) {
                (Some(Some(before)), Some(now)) if same(before, now) => alike += 1,
                (Some(Some(_)), Some(_)) => return false,
                _ => {}
            }
        }
        alike >= needed
    };
    let far = before.len().max(now.len()) as isize;
    let moves = (0..far).flat_map(|by| [by, -by]).skip(1).filter(fits);
    moves.filter(|_| needed > 0).collect()
}

/// A line of the list's picture: how bright its parts are, from the left (see
/// `list::picture`).
pub type Band = [u8; 12];

/// How unlike two pictures of the list are if the list moved `down` lines down the screen
/// between them: the mean difference of the lines both show, `None` if they share fewer
/// than `SHARED`.
pub fn unlike(before: &[Band], now: &[Band], down: f32) -> Option<f32> {
    let down = down.round() as isize;
    let pairs = now.iter().enumerate().filter_map(|(at, now)| {
        let before = before.get(usize::try_from(at as isize - down).ok()?)?;
        Some(
            before
                .iter()
                .zip(now)
                .map(|(a, b)| u32::from(a.abs_diff(*b))),
        )
    });
    let (mut sum, mut count) = (0u32, 0u32);
    for pair in pairs {
        sum += pair.sum::<u32>();
        count += 1;
    }
    (count >= SHARED).then(|| sum as f32 / (count * 12) as f32)
}

/// Two pictures compare over this many lines at least: a row's height.
const SHARED: u32 = 165;
/// A move's pictures are clearly more alike than another's at this much of its unlikeness.
const CLEARLY: f32 = 0.6;

/// Which of the moves the rows allow (each with how far down it takes the rows) the list
/// made: the only one; of several, the one whose pictures are clearly more alike than any
/// other's (the rows cut by the list's edges are in them, and tell where rows in full sight
/// do not). `None` where the pictures do not tell either.
pub fn moved(moves: &[(isize, f32)], before: &[Band], now: &[Band]) -> Option<isize> {
    if let [(only, _)] = moves {
        return Some(*only);
    }
    let mut seen: Vec<(f32, isize)> = moves
        .iter()
        .filter_map(|(by, down)| Some((unlike(before, now, *down)?, *by)))
        .collect();
    seen.sort_by(|a, b| a.0.total_cmp(&b.0));
    match seen[..] {
        [(least, by), (next, _), ..] if least <= next * CLEARLY && next > 0.0 => Some(by),
        _ => None,
    }
}

/// Whether the list stood still between two frames though a row reads otherwise: the rows
/// at the same tops (`tops`: before and now, a pixel off at most), one of them alike at
/// least and one not alike at most. A press draws a ring over the row pressed, and the row
/// does not read, or reads otherwise, while the ring spreads.
pub fn still<T>(
    tops: (&[f32], &[f32]),
    before: &[Option<T>],
    now: &[Option<T>],
    same: impl Fn(&T, &T) -> bool,
) -> bool {
    let level = |(before, now): (&f32, &f32)| (before - now).abs() <= 1.0;
    let alike = |(before, now): (&Option<T>, &Option<T>)| match (before, now) {
        (Some(before), Some(now)) => same(before, now),
        _ => false,
    };
    let alike = before.iter().zip(now).filter(|pair| alike(*pair)).count();
    tops.0.len() == tops.1.len()
        && tops.0.iter().zip(tops.1).all(level)
        && before.len() == now.len()
        && alike >= 1
        && alike + 1 >= now.len()
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
    /// The row at this index, by the ring the game draws where a press lands: the press
    /// that opened the record, whatever the pointer is over.
    Pressed(usize),
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
    /// Whether the row is surely this one (`Match::Kept`), or only likely.
    pub sure: bool,
    pub fight: Fight,
    pub battle: Option<i64>,
}

/// The row a record was opened from: the rows the list showed last (`None` for a row not
/// told), what the pointer was over then, the fight the record shows and the battle's
/// number. The row the press lay on, where the game's ring showed it, if it tells the
/// record's fight, and no other. Without the ring, of the rows that tell the record's
/// fight: the one under the pointer; or else the one that has this record already; or else
/// the first that has none. A touch moves no pointer, so the pointer only chooses among
/// rows that tell the same; a row that has another record is taken only from under the
/// pointer.
pub fn opened(rows: &[Option<Told>], pointed: Pointed, fight: Fight, battle: i64) -> Option<i64> {
    if let Pointed::Pressed(at) = pointed {
        let pressed = rows.get(at)?.as_ref().filter(|row| row.fight == fight);
        return pressed.map(|row| row.id);
    }
    let told = rows
        .iter()
        .enumerate()
        .filter_map(|(at, row)| Some((at, row.as_ref()?)));
    let like: Vec<(usize, &Told)> = told.filter(|(_, row)| row.fight == fight).collect();
    let pointed = like.iter().find(|(at, _)| pointed == Pointed::Row(*at));
    let own = like.iter().find(|(_, row)| row.battle == Some(battle));
    let free = like.iter().find(|(_, row)| row.battle.is_none());
    pointed.or(own).or(free).map(|(_, row)| row.id)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{
        Band, Fight, Listed, Match, Pointed, Shows, Sighting, Told, Up, align, moved, narrowed,
        opened, pointed, shifts, still, unlike,
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

    /// The rows kept, numbered from 100 on: runs of rows seen side by side, each from its
    /// top, a row an opponent and the end of its minute.
    fn kept(runs: &[&[(i64, i64)]]) -> Vec<Listed> {
        let mut rows: Vec<Listed> = Vec::new();
        for run in runs {
            for (at, (opponent, time)) in run.iter().enumerate() {
                let id = 100 + rows.len() as i64;
                rows.push(Listed {
                    id,
                    above: (at > 0).then_some(id - 1),
                    fight: fight(*opponent),
                    range: (time - 60, *time),
                    battle: None,
                });
            }
        }
        rows
    }

    /// The rows in sight told with none carried from a frame ago.
    fn fresh(kept: &[Listed], seen: &[Option<Sighting>]) -> Vec<Match> {
        align(kept, seen, &vec![None; seen.len()])
    }

    use Match::{Kept, Likely, New, Unsure};

    #[test]
    fn rows_never_seen_are_new() {
        assert_eq!(fresh(&[], &[seen(1, 9000), seen(2, 8000)]), [New, New]);
    }

    #[test]
    fn rows_side_by_side_are_the_rows_kept_side_by_side() {
        let kept = kept(&[&[(1, 9000), (2, 8000), (3, 7000)]]);
        assert_eq!(
            fresh(&kept, &[seen(1, 9000), seen(2, 8000), seen(3, 7000)]),
            [Kept(0), Kept(1), Kept(2)]
        );
        // The list scrolled: the rows further down.
        assert_eq!(
            fresh(&kept, &[seen(2, 8000), seen(3, 7000)]),
            [Kept(1), Kept(2)]
        );
        // A battle since: one more row on top.
        assert_eq!(
            fresh(&kept, &[seen(4, 9900), seen(1, 9000), seen(2, 8000)]),
            [New, Kept(0), Kept(1)]
        );
        // Rows under the last kept.
        assert_eq!(
            fresh(&kept, &[seen(2, 8000), seen(3, 7000), seen(4, 6000)]),
            [Kept(1), Kept(2), New]
        );
    }

    #[test]
    fn a_row_alone_among_new_ones_is_only_likely() {
        // Two battles since, and the row that was on top: it may be that row, or another
        // battle against the same opponent.
        let kept = kept(&[&[(1, 9000), (2, 8000)]]);
        let rows = [seen(4, 9900), seen(5, 9800), seen(1, 9000)];
        assert_eq!(fresh(&kept, &rows), [New, New, Likely(0)]);
        // Told a frame ago, the rows above it are sure; it is as likely as it was.
        let two = self::kept(&[&[(1, 9000), (2, 8000)], &[(4, 9900), (5, 9800)]]);
        assert_eq!(
            align(&two, &rows, &[Some(102), Some(103), None]),
            [Kept(2), Kept(3), Likely(0)]
        );
    }

    #[test]
    fn a_row_told_a_frame_ago_is_that_row() {
        let kept = kept(&[&[(1, 9000), (2, 8000), (3, 7000)]]);
        // The list moved on to the last row kept and rows never seen.
        let rows = [seen(3, 7000), seen(4, 6000), seen(5, 5000)];
        assert_eq!(fresh(&kept, &rows), [Likely(2), New, New]);
        assert_eq!(
            align(&kept, &rows, &[Some(102), None, None]),
            [Kept(2), New, New]
        );
        // A row told a frame ago that tells another fight now is not told anew: the list
        // does not change under Rio's eyes, so the reading is off (a press draws a ring
        // over the row pressed).
        assert_eq!(
            align(&kept, &rows, &[Some(100), None, None]),
            [Unsure(None), New, New]
        );
    }

    #[test]
    fn a_row_kept_alone_is_told_by_itself() {
        // No row was ever seen beside it: there is nothing beside it to go by.
        let one = kept(&[&[(1, 9000)]]);
        assert_eq!(fresh(&one, &[seen(1, 9000)]), [Kept(0)]);
        assert_eq!(fresh(&one, &[seen(2, 9000)]), [New], "another opponent");
        assert_eq!(fresh(&one, &[seen(1, 5000)]), [New], "another time");
        assert_eq!(
            fresh(&one, &[seen(1, 9100)]),
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
        assert_eq!(fresh(&one, &[lost]), [New], "another winner");
        // The opponent beaten again within the minute: the newer battle stands on top.
        assert_eq!(fresh(&one, &[seen(1, 9000), seen(1, 9000)]), [New, Kept(0)]);
        // Two kept alone that tell the same: the one kept first stands on top.
        let two = kept(&[&[(1, 9000)], &[(1, 9000)]]);
        assert_eq!(
            fresh(&two, &[seen(1, 9000), seen(1, 9000)]),
            [Kept(0), Kept(1)]
        );
    }

    #[test]
    fn a_row_is_not_taken_for_one_of_another_day_whose_neighbours_differ() {
        // Seen on 2026-11-18: the user fought the players 22 and 18 on two days running.
        // The rows "4日前" were kept; the list, open for half an hour, then showed the rows
        // "3日前", whose times reach those of the rows a day older.
        let older: Vec<Listed> = [22, 18, 23]
            .into_iter()
            .enumerate()
            .map(|(at, opponent)| Listed {
                id: 100 + at as i64,
                above: (at > 0).then(|| 99 + at as i64),
                fight: fight(opponent),
                range: (500_000 - 86400, 500_016),
                battle: None,
            })
            .collect();
        let day = |opponent| {
            Some(Sighting {
                fight: fight(opponent),
                range: (500_000, 588_200),
            })
        };
        let newer = [day(17), day(18), day(30)];
        assert_eq!(fresh(&older, &newer), [New, New, New]);
        // Kept alone, the older row would be taken for the newer: of two kept alone, the
        // one whose time fits better is.
        let alone = [
            Listed {
                above: None,
                ..older[1]
            },
            Listed {
                id: 200,
                above: None,
                fight: fight(18),
                range: (500_000, 588_200),
                battle: Some(7),
            },
        ];
        assert_eq!(fresh(&alone, &newer[1..2]), [Kept(1)]);
    }

    #[test]
    fn two_rows_that_tell_the_same_are_told_by_the_rows_beside_them() {
        // The same opponent beaten twice within the minute, then another.
        let twins = kept(&[&[(1, 9000), (1, 9000), (2, 8000)]]);
        assert_eq!(
            fresh(&twins, &[seen(1, 9000), seen(2, 8000)]),
            [Kept(1), Kept(2)],
            "the one just above the row below"
        );
        assert_eq!(
            fresh(&twins, &[seen(1, 9000), seen(1, 9000)]),
            [Kept(0), Kept(1)],
            "both in sight"
        );
        assert_eq!(
            fresh(&twins, &[seen(1, 9000)]),
            [Unsure(Some(false))],
            "alone in sight, it may be either: neither was read"
        );
        let twins = kept(&[&[(2, 9900), (1, 9000), (1, 9000)]]);
        assert_eq!(
            fresh(&twins, &[seen(2, 9900), seen(1, 9000)]),
            [Kept(0), Kept(1)],
            "the one just under the row above"
        );
    }

    #[test]
    fn a_row_that_may_be_either_of_two_is_read_where_both_are() {
        let mut twins = kept(&[&[(1, 9000), (1, 9000), (2, 8000)]]);
        twins[0].battle = Some(7);
        assert_eq!(fresh(&twins, &[seen(1, 9000)]), [Unsure(None)]);
        twins[1].battle = Some(8);
        assert_eq!(fresh(&twins, &[seen(1, 9000)]), [Unsure(Some(true))]);
    }

    #[test]
    fn a_new_row_like_one_kept_stands_above_it() {
        // The opponent beaten again: the row kept is the older of the two.
        let kept = kept(&[&[(1, 9000), (2, 8000)]]);
        assert_eq!(
            fresh(&kept, &[seen(1, 9000), seen(1, 9000), seen(2, 8000)]),
            [New, Kept(0), Kept(1)]
        );
    }

    #[test]
    fn a_row_that_did_not_read_is_not_told() {
        let kept = kept(&[&[(1, 9000), (2, 8000), (3, 7000)]]);
        assert_eq!(
            fresh(&kept, &[seen(1, 9000), seen(2, 8000), None]),
            [Kept(0), Kept(1), Unsure(None)]
        );
        // The rows on either side of it tell each other.
        assert_eq!(
            fresh(&kept, &[seen(1, 9000), None, seen(3, 7000)]),
            [Kept(0), Unsure(None), Kept(2)]
        );
    }

    #[test]
    fn rows_side_by_side_are_two_kept_only_where_the_rows_beyond_them_fit_too() {
        // The rows of the players 5, 6 and 7 were kept; the list shows 5, 6 and 8: other
        // rows (the players 5 and 6 fought again).
        let kept = kept(&[&[(5, 9000), (6, 8990), (7, 8980)]]);
        assert_eq!(
            fresh(&kept, &[seen(5, 9000), seen(6, 8990), seen(8, 8980)]),
            [New, New, New]
        );
    }

    #[test]
    fn rows_the_rows_kept_tell_are_not_told_otherwise_by_what_was_carried() {
        // Seen on 2026-11-18: the player 3 beaten three times, between two other rows.
        let kept = kept(&[&[(6, 9100), (3, 9000), (3, 9000), (3, 9000), (7, 8000)]]);
        let rows = [seen(3, 9000), seen(3, 9000), seen(3, 9000)];
        assert_eq!(fresh(&kept, &rows), [Kept(1), Kept(2), Kept(3)]);
        assert_eq!(
            align(&kept, &rows, &[Some(102), Some(103), None]),
            [Kept(1), Kept(2), Kept(3)],
            "carried one off, the third would be a fourth such row"
        );
        // Two of the three may be either two: there, what was carried tells.
        assert_eq!(
            fresh(&kept, &rows[..2]),
            [Unsure(Some(false)), Unsure(Some(false))]
        );
        assert_eq!(
            align(&kept, &rows[..2], &[Some(102), Some(103)]),
            [Kept(2), Kept(3)]
        );
    }

    #[test]
    fn a_row_kept_twice_is_the_one_of_the_run_with_more_records() {
        // Seen on 2026-11-18: rows told one off were kept anew, and two runs tell the same
        // rows at the same time.
        let mut twice = kept(&[
            &[(4, 9000), (5, 8000), (6, 7000)],
            &[(4, 9000), (5, 8000), (6, 7000), (3, 6000)],
        ]);
        twice[5].battle = Some(6);
        let rows = [seen(4, 9000), seen(5, 8000), seen(6, 7000)];
        assert_eq!(fresh(&twice, &rows), [Kept(3), Kept(4), Kept(5)]);
        assert_eq!(fresh(&twice, &rows[..1]), [Likely(3)], "alone in sight too");
        // Rows that tell the same at another time are other rows: either may be in sight.
        let other = kept(&[&[(4, 9000), (5, 8000)], &[(4, 9050), (5, 8050)]]);
        assert_eq!(
            fresh(&other, &[seen(4, 9020), seen(5, 8020)]),
            [Unsure(Some(false)), Unsure(Some(false))]
        );
    }

    #[test]
    fn two_runs_meet_where_a_row_of_the_one_was_told_a_frame_ago() {
        let kept = kept(&[&[(1, 9000), (2, 8000)], &[(3, 7000), (4, 6000)]]);
        let rows = [seen(2, 8000), seen(3, 7000), seen(4, 6000)];
        assert_eq!(fresh(&kept, &rows), [Likely(1), Kept(2), Kept(3)]);
        assert_eq!(
            align(&kept, &rows, &[Some(101), None, None]),
            [Kept(1), Kept(2), Kept(3)]
        );
    }

    #[test]
    fn a_row_is_not_the_row_kept_under_another_where_it_tells_another_fight() {
        // The rows 1, 2, 3 were kept side by side, wrongly: the list shows 4 under 1.
        let kept = kept(&[&[(1, 9000), (2, 8000), (3, 7000)]]);
        let rows = [seen(1, 9000), seen(4, 8500)];
        assert_eq!(align(&kept, &rows, &[Some(100), None]), [Kept(0), New]);
        // With nothing carried, what was kept counts: the row above is not the row kept.
        assert_eq!(fresh(&kept, &rows), [New, New]);
    }

    #[test]
    fn a_row_is_not_a_row_kept_that_stands_elsewhere_in_the_run_of_a_row_told() {
        // The players 5 and 6 fought a day ago and two days ago, in the same order. The rows
        // of two days ago were kept under the row of player 7; the list moves up to the
        // rows of a day ago, which stand above that row.
        let kept = kept(&[&[(7, 9000), (5, 8990), (6, 8980)]]);
        let rows = [seen(5, 9050), seen(6, 9040), seen(7, 9000)];
        assert_eq!(
            align(&kept, &rows, &[None, None, Some(100)]),
            [New, New, Kept(0)]
        );
    }

    #[test]
    fn the_list_moves_by_the_rows_two_frames_share() {
        let same = |a: &u8, b: &u8| a == b;
        let rows = |rows: &[u8]| rows.iter().map(|row| Some(*row)).collect::<Vec<_>>();
        let moves = |before: &[u8], now: &[u8]| shifts(&rows(before), &rows(now), same);
        assert_eq!(moves(&[1, 2, 3], &[1, 2, 3]), [0]);
        assert_eq!(moves(&[1, 2, 3], &[2, 3, 4]), [1]);
        assert_eq!(moves(&[2, 3, 4], &[1, 2, 3]), [-1]);
        assert_eq!(moves(&[1, 2, 3], &[2, 3]), [1], "a row left full sight");
        assert_eq!(moves(&[2, 3], &[2, 3, 4]), [0], "a row came into it");
        assert!(
            moves(&[1, 2, 3], &[3, 4, 5]).is_empty(),
            "one row is too little"
        );
        assert!(moves(&[1, 2, 3], &[4, 5, 6]).is_empty());
        assert_eq!(moves(&[1], &[1]), [0], "a list of one row");
        assert!(moves(&[], &[1]).is_empty());
        // Twins: the rows beside them tell how far.
        assert_eq!(moves(&[7, 7, 3], &[7, 3, 4]), [1]);
        // Rows that tell the same and no other in sight do not tell how far. Seen on
        // 2026-11-18: two of three such rows, then all three, were taken for no move, and
        // the rows were told one off.
        assert_eq!(moves(&[7, 7, 7], &[7, 7, 7]), [0, 1, -1]);
        assert_eq!(moves(&[7, 7], &[7, 7, 7]), [0, -1]);
        // A row that did not read in one of the frames is no row against it.
        assert_eq!(
            shifts(&[Some(1), None, Some(3)], &rows(&[1, 2, 3]), same),
            [0]
        );
        assert_eq!(shifts(&[Some(1), None], &rows(&[1, 2]), same), [0]);
    }

    /// The picture of a list scrolled `down` lines: a row is 175 lines, 165 as bright as
    /// its number says and 10 white ones.
    fn picture(rows: &[u8], down: usize) -> Vec<Band> {
        let line = |at: usize| match at % 175 {
            165.. => [255; 12],
            _ => [rows[at / 175]; 12],
        };
        (down..down + 662).map(line).collect()
    }

    #[test]
    fn of_the_moves_the_rows_allow_the_pictures_tell_the_one() {
        // Three rows that tell the same (70) between others. The list showed the second and
        // the third in full, and moved 100 lines down the screen: all three show now. By
        // the rows, it moved 100 lines down or 75 up.
        let rows = [10, 70, 70, 70, 20, 30, 40];
        let (before, now) = (picture(&rows, 250), picture(&rows, 150));
        assert_eq!(unlike(&before, &now, 100.0), Some(0.0));
        assert!(unlike(&before, &now, -75.0).is_some_and(|unlike| unlike > 5.0));
        assert_eq!(unlike(&before, &now, 600.0), None, "too little shared");
        assert_eq!(moved(&[(0, -75.0), (-1, 100.0)], &before, &now), Some(-1));
        // Among rows that all tell the same, the pictures tell no more than the rows.
        let rows = [70; 7];
        let (before, now) = (picture(&rows, 250), picture(&rows, 150));
        assert_eq!(moved(&[(0, -75.0), (-1, 100.0)], &before, &now), None);
        // One move allowed is the move; none is none.
        assert_eq!(moved(&[(1, 30.0)], &[], &[]), Some(1));
        assert_eq!(moved(&[], &before, &now), None);
    }

    #[test]
    fn the_list_stands_still_where_its_rows_keep_their_tops() {
        let same = |a: &u8, b: &u8| a == b;
        let tops = [398.0, 572.0, 747.0];
        let rows = [Some(1), Some(2), Some(3)];
        let stood =
            |tops_now: &[f32], now: &[Option<u8>]| still((&tops[..], tops_now), &rows, now, same);
        assert!(
            stood(&[398.4, 572.3, 747.0], &[Some(1), None, Some(3)]),
            "the row pressed does not read"
        );
        assert!(
            stood(&tops, &[Some(1), Some(9), Some(3)]),
            "or reads otherwise"
        );
        assert!(
            !stood(&tops, &[Some(1), Some(9), Some(8)]),
            "two rows differ"
        );
        assert!(
            !stood(&[393.0, 567.0, 742.0], &[Some(1), None, Some(3)]),
            "it moved"
        );
        assert!(!stood(&tops[..2], &rows[..2]), "fewer rows");
        assert!(!stood(&tops, &[None, None, None]), "no row to go by");
        // Two rows in sight, the one pressed.
        assert!(still(
            (&tops[..2], &tops[..2]),
            &rows[..2],
            &[Some(1), Some(7)],
            same
        ));
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
            sure: true,
            fight: fight(opponent),
            battle,
        })
    }

    #[test]
    fn a_record_is_the_row_that_tells_its_fight() {
        let rows = [told(11, 1, None), told(12, 2, None), None];
        assert_eq!(opened(&rows, Pointed::Row(1), fight(2), 7), Some(12));
        assert_eq!(opened(&rows, Pointed::Unknown, fight(2), 7), Some(12));
        // A touch moves no pointer: where the pointer lies says nothing against the row.
        assert_eq!(opened(&rows, Pointed::Row(0), fight(2), 7), Some(12));
        assert_eq!(opened(&rows, Pointed::Elsewhere, fight(2), 7), Some(12));
        assert_eq!(
            opened(&rows, Pointed::Unknown, fight(3), 7),
            None,
            "no such row"
        );
        // A row that has another record is the record's only from under the pointer: the
        // record may be of a row cut by the list's edge that tells the same.
        let read = [told(11, 1, Some(5))];
        assert_eq!(opened(&read, Pointed::Unknown, fight(1), 7), None);
        assert_eq!(opened(&read, Pointed::Row(0), fight(1), 7), Some(11));
        assert_eq!(opened(&read, Pointed::Unknown, fight(1), 5), Some(11));
    }

    #[test]
    fn a_record_is_the_row_the_press_lay_on() {
        let twins = [told(11, 1, None), told(12, 1, None), None];
        assert_eq!(opened(&twins, Pointed::Pressed(1), fight(1), 7), Some(12));
        assert_eq!(
            opened(&twins, Pointed::Pressed(2), fight(1), 7),
            None,
            "a row not told: no other row is the record's"
        );
        assert_eq!(
            opened(&twins, Pointed::Pressed(0), fight(2), 7),
            None,
            "the row tells another fight"
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
