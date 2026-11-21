//! Rio as a server of the Model Context Protocol (`bluearchive-rio mcp`): an agent registers
//! the formations it reads of, changes them and takes them away, in the records the window
//! reads. JSON-RPC 2.0 over the standard streams, a message a line; no window opens.
//!
//! What a tool tells is JSON in a text: what was done, or `error` with why not (and
//! `candidates` where a student's name fits several or none).

use std::io::{BufRead, Write};

use serde_json::{Map, Value, json};

use crate::{
    Squad, Student, app,
    draft::takes,
    search::{self, Filter},
    source::{self, MOST, Reported, Source},
    store::{Amended, Registered, Store},
};

/// The protocol's versions answered as asked for; any other is answered with the first.
const VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// JSON-RPC's errors.
const PARSE: i64 = -32700;
const NO_REQUEST: i64 = -32600;
const NO_METHOD: i64 = -32601;
const PARAMS: i64 = -32602;

/// How many students a search tells at most.
const FOUND: usize = 50;
/// How many students are offered for a name that fits none.
const NEAR: usize = 10;

/// What a tool tells, or why it did nothing: both JSON, the client's to read.
type Told = Result<Value, Value>;

/// Answers the client's messages, a line each, until its stream ends.
pub fn serve(
    store: &Store,
    roster: &[Student],
    mut input: impl BufRead,
    mut output: impl Write,
) -> std::io::Result<()> {
    let mut line = Vec::new();
    loop {
        line.clear();
        if input.read_until(b'\n', &mut line)? == 0 {
            return Ok(());
        }
        if let Some(answer) = answer(store, roster, &String::from_utf8_lossy(&line)) {
            writeln!(output, "{answer}")?;
            output.flush()?;
        }
    }
}

/// The answer to a request: its result, or the error's code and what it says.
fn respond(id: Value, done: Result<Value, (i64, String)>) -> String {
    let answer = match done {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err((code, message)) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": code, "message": message },
        }),
    };
    answer.to_string()
}

/// The answer to one message of the client (a line), as a line; none to a notification, a
/// response or an empty line.
pub fn answer(store: &Store, roster: &[Student], line: &str) -> Option<String> {
    if line.trim().is_empty() {
        return None;
    }
    let Ok(message) = serde_json::from_str::<Value>(line) else {
        let unread = Err((PARSE, "Parse error".to_owned()));
        return Some(respond(Value::Null, unread));
    };
    let id = message.get("id").cloned();
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        // A response to something Rio never asked is let pass; anything else is no request.
        let response = message.get("result").is_some() || message.get("error").is_some();
        let no_request = Err((NO_REQUEST, "Invalid Request".to_owned()));
        return (!response).then(|| respond(id.unwrap_or(Value::Null), no_request));
    };
    // A notification asks for no answer.
    let id = id?;
    let params = message.get("params").cloned().unwrap_or(Value::Null);
    let done = match method {
        "initialize" => Ok(initialize(&params)),
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools() })),
        "tools/call" => call(store, roster, &params),
        _ => Err((NO_METHOD, format!("Method not found: {method}"))),
    };
    Some(respond(id, done))
}

/// What Rio is and can do, in the protocol's version the client asked for where Rio knows it.
fn initialize(params: &Value) -> Value {
    let asked = params.get("protocolVersion").and_then(Value::as_str);
    let version = asked
        .filter(|asked| VERSIONS.contains(asked))
        .unwrap_or(VERSIONS[0]);
    json!({
        "protocolVersion": version,
        "capabilities": { "tools": {} },
        "serverInfo": { "name": "bluearchive-rio", "version": app::VERSION },
        "instructions": "ブルーアーカイブの戦術対抗戦で「この防衛にはこの攻め」と紹介された編成を、Rio に登録・編集・削除します。生徒は名前か ID で指定します。同じ名前の生徒がいるときは find_students で ID を調べます。",
    })
}

/// The tools, as the client lists them.
fn tools() -> Value {
    let student = json!({
        "type": ["string", "integer"],
        "description": "生徒の名前（例: ホシノ（水着））か、SchaleDB の ID（例: 10045）",
    });
    let team = |side: &str, place: &str| {
        json!({
            "type": "array",
            "minItems": 6,
            "maxItems": 6,
            "items": student,
            "description": format!(
                "{side}の 6 人。ストライカー 4 人（{place}1〜{place}4、左から）、スペシャル 2 人の順"
            ),
        })
    };
    let battles = format!("紹介された戦績の戦数（1〜{MOST}）。wins と一緒に指定する");
    let wins = "紹介された戦績の勝数（0〜battles）";
    let url = "元になったページの URL（http:// か https://）";
    let id = json!({ "type": "integer", "description": "登録の ID（list_formations で分かる）" });
    json!([
        {
            "name": "find_students",
            "description": "生徒を名前の一部で探す（ひらがなでもよい）。同じ名前の生徒の ID を調べるのに使う。",
            "inputSchema": {
                "type": "object",
                "properties": { "name": { "type": "string", "description": "名前の一部" } },
                "required": ["name"],
            },
        },
        {
            "name": "list_formations",
            "description": "登録した編成（攻めと防衛の組）を、新しい順にすべて返す。",
            "inputSchema": { "type": "object", "properties": {} },
        },
        {
            "name": "register_formation",
            "description": "「この防衛にはこの攻め」を 1 組登録する。紹介された戦績と URL も付けられる。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "attack": team("攻め", "A"),
                    "defense": team("防衛", "D"),
                    "battles": { "type": "integer", "description": battles },
                    "wins": { "type": "integer", "description": wins },
                    "url": { "type": "string", "description": url },
                },
                "required": ["attack", "defense"],
            },
        },
        {
            "name": "edit_formation",
            "description": "登録した編成を変える。指定した項目だけが変わる。battles と wins、url は null で消せる。",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": id,
                    "attack": team("攻め", "A"),
                    "defense": team("防衛", "D"),
                    "battles": { "type": ["integer", "null"], "description": battles },
                    "wins": { "type": ["integer", "null"], "description": wins },
                    "url": { "type": ["string", "null"], "description": url },
                },
                "required": ["id"],
            },
        },
        {
            "name": "delete_formation",
            "description": "登録した編成を 1 組消す。",
            "inputSchema": {
                "type": "object",
                "properties": { "id": id },
                "required": ["id"],
            },
        },
    ])
}

/// Calls a tool: what it tells is a text of JSON, marked where it did nothing.
fn call(store: &Store, roster: &[Student], params: &Value) -> Result<Value, (i64, String)> {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return Err((PARAMS, "tools/call needs the tool's name".to_owned()));
    };
    let none = Map::new();
    let given = params.get("arguments").and_then(Value::as_object);
    let given = given.unwrap_or(&none);
    let told = match name {
        "find_students" => find(roster, given),
        "list_formations" => list(store, roster),
        "register_formation" => register(store, roster, given),
        "edit_formation" => edit(store, roster, given),
        "delete_formation" => delete(store, roster, given),
        _ => return Err((PARAMS, format!("Unknown tool: {name}"))),
    };
    let (text, refused) = match told {
        Ok(told) => (told, false),
        Err(why) => (why, true),
    };
    Ok(json!({
        "content": [{ "type": "text", "text": text.to_string() }],
        "isError": refused,
    }))
}

/// Why a tool did nothing, in words.
fn refusal(why: impl Into<String>) -> Value {
    json!({ "error": why.into() })
}

/// The records could not be read or written.
fn records(error: rusqlite::Error) -> Value {
    refusal(format!("記録を読み書きできません: {error}"))
}

/// A student as a tool tells of her: what tells her from another of the same name too.
fn told(student: &Student) -> Value {
    let squad = match student.squad {
        Squad::Main => "ストライカー",
        Squad::Support => "スペシャル",
    };
    json!({
        "id": student.id,
        "name": student.name,
        "squad": squad,
        "class": student.role.name(),
        "attack": student.attack.name(),
        "armor": student.armor.name(),
    })
}

/// A name as it is compared: however it is typed (`search::fold`), the brackets of half
/// width as the names' own of full width.
fn plain(name: &str) -> String {
    search::fold(name).replace('(', "（").replace(')', "）")
}

/// The student a client names: by the number, or by the whole name. A name several students
/// have, or none has, is refused with the students it may mean.
fn student<'a>(roster: &'a [Student], named: &Value) -> Result<&'a Student, Value> {
    let by_id = |id: i64| {
        let found = roster.iter().find(|student| student.id == id);
        found.ok_or_else(|| refusal(format!("ID {id} の生徒はいません")))
    };
    if let Some(id) = named.as_i64() {
        return by_id(id);
    }
    let Some(name) = named.as_str() else {
        return Err(refusal("生徒は名前（文字列）か ID（整数）で指定します"));
    };
    if let Ok(id) = name.trim().parse::<i64>() {
        return by_id(id);
    }
    let wanted = plain(name);
    let same: Vec<&Student> = (roster.iter())
        .filter(|student| plain(&student.name) == wanted)
        .collect();
    match same.as_slice() {
        [one] => Ok(one),
        [] => {
            let near = search::found(roster, &wanted, &Filter::default());
            let near: Vec<Value> = near.into_iter().take(NEAR).map(told).collect();
            Err(json!({
                "error": format!("「{name}」という名前の生徒はいません"),
                "candidates": near,
            }))
        }
        several => Err(json!({
            "error": format!(
                "「{name}」という名前の生徒は {} 人います。ID で指定してください",
                several.len()
            ),
            "candidates": several.iter().copied().map(told).collect::<Vec<_>>(),
        })),
    }
}

/// A team a client names under `key`: six students, the four strikers then the two
/// specials, each once.
fn team(
    roster: &[Student],
    given: &Map<String, Value>,
    (key, side, letter): (&str, &str, char),
) -> Result<[i64; 6], Value> {
    let six = || {
        refusal(format!(
            "{key} は 6 人の配列で指定します（ストライカー 4 人、スペシャル 2 人の順）"
        ))
    };
    let named = given.get(key).and_then(Value::as_array).ok_or_else(six)?;
    if named.len() != 6 {
        return Err(six());
    }
    let mut team = [0; 6];
    for (place, named) in named.iter().enumerate() {
        let student = student(roster, named)?;
        let name = &student.name;
        if student.squad != takes(place) {
            let (place, squad) = match takes(place) {
                Squad::Main => (format!("{letter}{}", place + 1), "スペシャル"),
                Squad::Support => ("SP".to_owned(), "ストライカー"),
            };
            return Err(refusal(format!(
                "{side}の {place} に {name} は入れられません（{squad}の生徒です）"
            )));
        }
        if team[..place].contains(&student.id) {
            return Err(refusal(format!("{side}に {name} が 2 回入っています")));
        }
        team[place] = student.id;
    }
    Ok(team)
}

/// The record a client gives: not given (`None`), taken away (`Some(None)`), or told.
fn reported(given: &Map<String, Value>) -> Result<Option<Option<Reported>>, Value> {
    let how = || {
        refusal(format!(
            "battles は 1〜{MOST}、wins は 0〜battles の整数で、両方指定します"
        ))
    };
    let figure = |value: &Value| value.as_u64().and_then(|figure| u32::try_from(figure).ok());
    match (given.get("battles"), given.get("wins")) {
        (None, None) => Ok(None),
        (Some(Value::Null), None | Some(Value::Null)) | (None, Some(Value::Null)) => Ok(Some(None)),
        (Some(battles), Some(wins)) => {
            let (battles, wins) = figure(battles).zip(figure(wins)).ok_or_else(how)?;
            let told = Reported::new(battles, wins).ok_or_else(how)?;
            Ok(Some(Some(told)))
        }
        _ => Err(how()),
    }
}

/// The page a client gives: not given, taken away (null or an empty text), or told.
fn page(given: &Map<String, Value>) -> Result<Option<Option<String>>, Value> {
    let how = || refusal("url は http:// か https:// で始まるアドレスで指定します");
    match given.get("url") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(None)),
        Some(Value::String(typed)) => source::page(typed).map(Some).map_err(|_| how()),
        Some(_) => Err(how()),
    }
}

/// The number of a registration a client names.
fn number(given: &Map<String, Value>) -> Result<i64, Value> {
    let id = given.get("id").and_then(Value::as_i64);
    id.ok_or_else(|| refusal("id は登録の ID（整数）で指定します"))
}

/// A registration as a tool tells of it.
fn formation(roster: &[Student], registered: &Registered) -> Value {
    let team = |team: &[i64; 6]| -> Vec<Value> {
        let named = |id: &i64| {
            let known = roster.iter().find(|student| student.id == *id);
            let name = known.map_or_else(|| format!("生徒#{id}"), |student| student.name.clone());
            json!({ "id": id, "name": name })
        };
        team.iter().map(named).collect()
    };
    let source = &registered.source;
    json!({
        "id": registered.id,
        "added_at": registered.added_at,
        "attack": team(&registered.attack),
        "defense": team(&registered.defense),
        "battles": source.reported.map(|told| told.battles),
        "wins": source.reported.map(|told| told.wins),
        "url": source.url,
    })
}

fn find(roster: &[Student], given: &Map<String, Value>) -> Told {
    let Some(name) = given.get("name").and_then(Value::as_str) else {
        return Err(refusal("name は名前の一部（文字列）で指定します"));
    };
    let found = search::found(roster, &plain(name), &Filter::default());
    let omitted = found.len().saturating_sub(FOUND);
    let students: Vec<Value> = found.into_iter().take(FOUND).map(told).collect();
    Ok(json!({ "students": students, "omitted": omitted }))
}

fn list(store: &Store, roster: &[Student]) -> Told {
    let kept = store.registered().map_err(records)?;
    let formations: Vec<Value> = (kept.iter())
        .map(|registered| formation(roster, registered))
        .collect();
    Ok(json!({ "formations": formations }))
}

fn register(store: &Store, roster: &[Student], given: &Map<String, Value>) -> Told {
    let attack = team(roster, given, ("attack", "攻め", 'A'))?;
    let defense = team(roster, given, ("defense", "防衛", 'D'))?;
    let source = Source {
        reported: reported(given)?.flatten(),
        url: page(given)?.flatten(),
    };
    let (id, new) = (store.register(attack, defense, &source)).map_err(records)?;
    if new {
        Ok(json!({ "id": id, "new": true }))
    } else {
        Ok(json!({
            "id": id,
            "new": false,
            "note": "同じ攻めと防衛は登録済みです。戦績や URL を変えるには edit_formation を使います。",
        }))
    }
}

fn edit(store: &Store, roster: &[Student], given: &Map<String, Value>) -> Told {
    let id = number(given)?;
    let gone = || refusal(format!("ID {id} の登録はありません"));
    let was = store.registration(id).map_err(records)?.ok_or_else(gone)?;
    let attack = match given.contains_key("attack") {
        true => team(roster, given, ("attack", "攻め", 'A'))?,
        false => was.attack,
    };
    let defense = match given.contains_key("defense") {
        true => team(roster, given, ("defense", "防衛", 'D'))?,
        false => was.defense,
    };
    let source = Source {
        reported: reported(given)?.unwrap_or(was.source.reported),
        url: page(given)?.unwrap_or(was.source.url),
    };
    match (store.amend(id, attack, defense, &source)).map_err(records)? {
        Amended::Done => {
            let now = store.registration(id).map_err(records)?.ok_or_else(gone)?;
            Ok(json!({ "formation": formation(roster, &now) }))
        }
        Amended::Gone => Err(gone()),
        Amended::Taken(other) => Err(refusal(format!(
            "同じ攻めと防衛の登録がもうあります（ID {other}）"
        ))),
    }
}

fn delete(store: &Store, roster: &[Student], given: &Map<String, Value>) -> Told {
    let id = number(given)?;
    let gone = || refusal(format!("ID {id} の登録はありません"));
    let was = store.forget(id).map_err(records)?.ok_or_else(gone)?;
    Ok(json!({ "deleted": formation(roster, &was) }))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::{Value, json};

    use super::{answer, serve};
    use crate::{
        Student,
        source::{Reported, Source},
        store::Store,
    };

    fn store() -> Store {
        Store::open(Path::new(":memory:")).unwrap()
    }

    fn roster() -> Vec<Student> {
        crate::students()
    }

    /// What Rio answers to a request.
    fn ask(store: &Store, request: Value) -> Value {
        let answer = answer(store, &roster(), &request.to_string());
        let answer = answer.unwrap_or_else(|| panic!("no answer to {request}"));
        assert!(!answer.contains('\n'), "an answer is one line");
        serde_json::from_str(&answer).unwrap()
    }

    /// Calls a tool: whether it did nothing, and what it tells.
    fn call(store: &Store, tool: &str, arguments: Value) -> (bool, Value) {
        let request = json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": { "name": tool, "arguments": arguments },
        });
        let answer = ask(store, request);
        assert_eq!(answer["id"], 7);
        let result = &answer["result"];
        assert_eq!(result["content"][0]["type"], "text", "{answer}");
        let text = result["content"][0]["text"].as_str().unwrap();
        let refused = result["isError"].as_bool().unwrap();
        (refused, serde_json::from_str(text).unwrap())
    }

    /// The call is refused, and nothing is registered: why.
    fn turned_down(store: &Store, tool: &str, arguments: Value) -> Value {
        let before = store.registered().unwrap();
        let (refused, why) = call(store, tool, arguments.clone());
        assert!(refused, "{tool} took {arguments}: {why}");
        assert!(why["error"].is_string(), "{why}");
        assert_eq!(store.registered().unwrap(), before, "nothing changes");
        why
    }

    /// ホシノ, マリナ, ミカ and イオリ with アコ and イロハ, by their names; and a defense by
    /// names and numbers, behind ユウカ.
    fn attack() -> Value {
        json!(["ホシノ", "マリナ", "ミカ", "イオリ", "イロハ", "アコ"])
    }

    fn defense() -> Value {
        json!([
            13010,
            "マリナ",
            "トキ",
            "イオリ",
            "イロハ",
            "ミチル（ドレス）"
        ])
    }

    const ATTACK: [i64; 6] = [10005, 10037, 10059, 10006, 20008, 20016];
    const DEFENSE: [i64; 6] = [13010, 10037, 10062, 10006, 20016, 20053];

    fn ids(students: &Value) -> Vec<i64> {
        let students = students.as_array().unwrap_or_else(|| panic!("{students}"));
        students
            .iter()
            .map(|student| student["id"].as_i64().unwrap())
            .collect()
    }

    #[test]
    fn rio_says_what_it_is_in_the_version_asked_for() {
        let store = store();
        let hello = |version: &str| {
            json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "initialize",
                "params": {
                    "protocolVersion": version,
                    "capabilities": {},
                    "clientInfo": { "name": "a client", "version": "1" },
                },
            })
        };
        let answer = ask(&store, hello("2025-03-26"));
        assert_eq!(
            (&answer["jsonrpc"], &answer["id"]),
            (&json!("2.0"), &json!(1))
        );
        let result = &answer["result"];
        assert_eq!(result["protocolVersion"], "2025-03-26", "as asked for");
        assert_eq!(result["serverInfo"]["name"], "bluearchive-rio");
        assert_eq!(result["serverInfo"]["version"], crate::app::VERSION);
        assert!(result["capabilities"]["tools"].is_object(), "it has tools");
        // A version Rio does not know: the latest it does.
        let answer = ask(&store, hello("1999-01-01"));
        assert_eq!(answer["result"]["protocolVersion"], "2025-06-18");
    }

    #[test]
    fn a_notification_is_not_answered_and_a_ping_is() {
        let (store, roster) = (store(), roster());
        let told = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        assert_eq!(answer(&store, &roster, &told.to_string()), None);
        let cancelled =
            json!({ "jsonrpc": "2.0", "method": "notifications/cancelled", "params": {} });
        assert_eq!(answer(&store, &roster, &cancelled.to_string()), None);
        assert_eq!(answer(&store, &roster, "  \r\n"), None, "an empty line");
        // A response to something Rio never asked.
        let response = json!({ "jsonrpc": "2.0", "id": 3, "result": {} });
        assert_eq!(answer(&store, &roster, &response.to_string()), None);
        let ping = ask(
            &store,
            json!({ "jsonrpc": "2.0", "id": "a", "method": "ping" }),
        );
        assert_eq!(ping, json!({ "jsonrpc": "2.0", "id": "a", "result": {} }));
    }

    #[test]
    fn what_is_no_request_is_answered_with_the_error() {
        let (store, roster) = (store(), roster());
        let unread = answer(&store, &roster, "{ not json").unwrap();
        let unread: Value = serde_json::from_str(&unread).unwrap();
        assert_eq!(
            (&unread["id"], &unread["error"]["code"]),
            (&Value::Null, &json!(-32700))
        );
        let unknown = ask(
            &store,
            json!({ "jsonrpc": "2.0", "id": 2, "method": "resources/list" }),
        );
        assert_eq!(
            (&unknown["id"], &unknown["error"]["code"]),
            (&json!(2), &json!(-32601))
        );
        assert!(unknown.get("result").is_none());
        let no_method = ask(&store, json!({ "jsonrpc": "2.0", "id": 4 }));
        assert_eq!(no_method["error"]["code"], -32600);
        // A tool Rio does not have, and a call that names none.
        let call = |params: Value| {
            let request =
                json!({ "jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": params });
            ask(&store, request)["error"]["code"].clone()
        };
        assert_eq!(
            call(json!({ "name": "drop_everything", "arguments": {} })),
            -32602
        );
        assert_eq!(call(json!({})), -32602);
    }

    #[test]
    fn the_tools_are_listed_with_what_each_takes() {
        let store = store();
        let listed = ask(
            &store,
            json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }),
        );
        let tools = listed["result"]["tools"].as_array().unwrap();
        let names: Vec<&str> = (tools.iter())
            .map(|tool| tool["name"].as_str().unwrap())
            .collect();
        assert_eq!(
            names,
            [
                "find_students",
                "list_formations",
                "register_formation",
                "edit_formation",
                "delete_formation"
            ]
        );
        for tool in tools {
            assert!(tool["description"].is_string(), "{tool}");
            assert_eq!(tool["inputSchema"]["type"], "object", "{tool}");
        }
        let register = &tools[2]["inputSchema"];
        assert_eq!(register["required"], json!(["attack", "defense"]));
        for taken in ["attack", "defense", "battles", "wins", "url"] {
            assert!(register["properties"][taken].is_object(), "{taken}");
        }
    }

    #[test]
    fn students_are_found_by_a_part_of_the_name() {
        let store = store();
        let (refused, found) = call(&store, "find_students", json!({ "name": "ほしの" }));
        assert!(!refused);
        assert_eq!(
            ids(&found["students"]),
            [10005, 10045, 10098, 10099],
            "by name: ホシノ, her swimsuit, and the two of the last stand"
        );
        // What tells the two of one name apart.
        let last_stand = &found["students"][2];
        assert_eq!(last_stand["name"], "ホシノ（臨戦）");
        assert_eq!(
            (&last_stand["squad"], &last_stand["class"]),
            (&json!("ストライカー"), &json!("タンク"))
        );
        assert_eq!(found["students"][3]["class"], "アタッカー");
        assert_eq!(found["omitted"], 0);
        // Many: the first fifty, and how many more.
        let (_, all) = call(&store, "find_students", json!({ "name": "" }));
        assert_eq!(all["students"].as_array().unwrap().len(), 50);
        assert_eq!(all["omitted"], roster().len() - 50);
        turned_down(&store, "find_students", json!({}));
    }

    #[test]
    fn a_formation_is_registered_by_names_and_numbers_with_what_it_was_told_with() {
        let store = store();
        let post = "https://x.com/someone/status/1";
        let given = json!({
            "attack": attack(),
            "defense": defense(),
            "battles": 20,
            "wins": 18,
            "url": post,
        });
        let (refused, done) = call(&store, "register_formation", given.clone());
        assert!(!refused, "{done}");
        assert_eq!(done["new"], true);
        let kept = store.registered().unwrap();
        assert_eq!(kept.len(), 1);
        assert_eq!(done["id"], kept[0].id);
        assert_eq!((kept[0].attack, kept[0].defense), (ATTACK, DEFENSE));
        let source = Source {
            reported: Reported::new(20, 18),
            url: Some(post.to_owned()),
        };
        assert_eq!(kept[0].source, source);
        // The same teams again: the registration there is, as it was.
        let again = json!({ "attack": attack(), "defense": defense() });
        let (refused, done) = call(&store, "register_formation", again);
        assert!(!refused);
        assert_eq!(
            (&done["id"], &done["new"]),
            (&json!(kept[0].id), &json!(false))
        );
        assert_eq!(store.registered().unwrap(), kept);
        // With nothing told, and the brackets of half width.
        let bare = json!({
            "attack": ["ホシノ(水着)", "マリナ", "ミカ", "イオリ", "イロハ", "アコ"],
            "defense": defense(),
        });
        let (refused, done) = call(&store, "register_formation", bare);
        assert!(!refused, "{done}");
        let kept = store.registered().unwrap();
        assert_eq!(kept[0].attack[0], 10045);
        assert_eq!(kept[0].source, Source::default());
    }

    #[test]
    fn a_name_of_several_students_or_of_none_is_refused_with_whom_it_may_mean() {
        let store = store();
        let with = |first: Value| {
            let attack = json!([first, "マリナ", "ミカ", "イオリ", "イロハ", "アコ"]);
            json!({ "attack": attack, "defense": defense() })
        };
        let several = turned_down(&store, "register_formation", with(json!("ホシノ（臨戦）")));
        assert_eq!(ids(&several["candidates"]), [10098, 10099]);
        assert_eq!(several["candidates"][1]["class"], "アタッカー");
        // By the number, she is the one meant.
        let (refused_by_id, _) = call(&store, "register_formation", with(json!(10099)));
        assert!(!refused_by_id);
        assert_eq!(store.registered().unwrap()[0].attack[0], 10099);
        // A part of a name is nobody's name: those it may mean.
        let part = turned_down(&store, "register_formation", with(json!("ホシ")));
        assert_eq!(ids(&part["candidates"]), [10005, 10045, 10098, 10099]);
        let nobody = turned_down(&store, "register_formation", with(json!("ここにいない")));
        assert_eq!(nobody["candidates"], json!([]));
        turned_down(&store, "register_formation", with(json!(99999)));
        turned_down(&store, "register_formation", with(json!(true)));
    }

    #[test]
    fn a_team_that_cannot_be_is_refused() {
        let store = store();
        let with = |attack: Value| json!({ "attack": attack, "defense": defense() });
        // A special where a striker stands, a striker where a special stands.
        let special = with(json!([
            "アコ",
            "マリナ",
            "ミカ",
            "イオリ",
            "イロハ",
            "アコ"
        ]));
        let why = turned_down(&store, "register_formation", special);
        assert!(why["error"].as_str().unwrap().contains("A1"), "{why}");
        let striker = with(json!([
            "ホシノ",
            "マリナ",
            "ミカ",
            "イオリ",
            "イロハ",
            "トキ"
        ]));
        let why = turned_down(&store, "register_formation", striker);
        assert!(why["error"].as_str().unwrap().contains("SP"), "{why}");
        // A student twice, five students, no team.
        let twice = with(json!([
            "ホシノ",
            "マリナ",
            "ホシノ",
            "イオリ",
            "イロハ",
            "アコ"
        ]));
        turned_down(&store, "register_formation", twice);
        let five = with(json!(["ホシノ", "マリナ", "ミカ", "イオリ", "イロハ"]));
        turned_down(&store, "register_formation", five);
        turned_down(&store, "register_formation", json!({ "attack": attack() }));
    }

    #[test]
    fn a_record_or_a_page_that_cannot_be_is_refused() {
        let store = store();
        let with = |more: Value| {
            let mut given = json!({ "attack": attack(), "defense": defense() });
            for (key, value) in more.as_object().unwrap() {
                given[key] = value.clone();
            }
            given
        };
        for more in [
            json!({ "battles": 18, "wins": 20 }),
            json!({ "battles": 20 }),
            json!({ "wins": 3 }),
            json!({ "battles": 0, "wins": 0 }),
            json!({ "battles": "20", "wins": "18" }),
            json!({ "battles": -1, "wins": 0 }),
            json!({ "battles": 10000, "wins": 1 }),
            json!({ "url": "file:///C:/Windows/notepad.exe" }),
            json!({ "url": 5 }),
        ] {
            turned_down(&store, "register_formation", with(more));
        }
    }

    #[test]
    fn the_formations_registered_are_listed_with_the_students_names() {
        let store = store();
        let (_, none) = call(&store, "list_formations", json!({}));
        assert_eq!(none["formations"], json!([]));
        let source = Source {
            reported: Reported::new(20, 18),
            url: Some("https://example.com/a".to_owned()),
        };
        let (first, _) = store.register(ATTACK, DEFENSE, &source).unwrap();
        let mut other = ATTACK;
        other[0] = 10045;
        let (second, _) = store.register(other, DEFENSE, &Source::default()).unwrap();
        let (refused, listed) = call(&store, "list_formations", json!({}));
        assert!(!refused);
        let listed = listed["formations"].as_array().unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(
            (&listed[0]["id"], &listed[1]["id"]),
            (&json!(second), &json!(first))
        );
        assert_eq!(ids(&listed[1]["attack"]), ATTACK);
        assert_eq!(ids(&listed[1]["defense"]), DEFENSE);
        assert_eq!(listed[1]["attack"][0]["name"], "ホシノ");
        assert_eq!(listed[1]["defense"][5]["name"], "ミチル（ドレス）");
        assert_eq!(
            (&listed[1]["battles"], &listed[1]["wins"], &listed[1]["url"]),
            (&json!(20), &json!(18), &json!("https://example.com/a"))
        );
        assert_eq!(
            (&listed[0]["battles"], &listed[0]["wins"], &listed[0]["url"]),
            (&Value::Null, &Value::Null, &Value::Null),
            "nothing was told with it"
        );
        assert!(listed[0]["added_at"].is_string());
    }

    #[test]
    fn a_registration_is_changed_in_what_is_given_alone() {
        let store = store();
        let source = Source {
            reported: Reported::new(20, 18),
            url: Some("https://example.com/a".to_owned()),
        };
        let (id, _) = store.register(ATTACK, DEFENSE, &source).unwrap();
        let now = |store: &Store| store.registration(id).unwrap().unwrap();
        // The record alone: the teams and the page stay.
        let (refused, done) = call(
            &store,
            "edit_formation",
            json!({ "id": id, "battles": 10, "wins": 7 }),
        );
        assert!(!refused, "{done}");
        assert_eq!(
            (&done["formation"]["battles"], &done["formation"]["wins"]),
            (&json!(10), &json!(7))
        );
        let kept = now(&store);
        assert_eq!((kept.attack, kept.defense), (ATTACK, DEFENSE));
        assert_eq!(kept.source.reported, Reported::new(10, 7));
        assert_eq!(kept.source.url, source.url);
        // The page taken away, then the record; then a page given again.
        call(&store, "edit_formation", json!({ "id": id, "url": null }));
        assert_eq!(now(&store).source.url, None);
        assert_eq!(now(&store).source.reported, Reported::new(10, 7));
        call(
            &store,
            "edit_formation",
            json!({ "id": id, "battles": null, "wins": null }),
        );
        assert_eq!(now(&store).source, Source::default());
        call(
            &store,
            "edit_formation",
            json!({ "id": id, "url": "https://example.com/b" }),
        );
        assert_eq!(
            now(&store).source.url.as_deref(),
            Some("https://example.com/b")
        );
        // Another attack: the defense stays.
        let swimsuit = json!([
            "ホシノ（水着）",
            "マリナ",
            "ミカ",
            "イオリ",
            "イロハ",
            "アコ"
        ]);
        let (refused, _) = call(
            &store,
            "edit_formation",
            json!({ "id": id, "attack": swimsuit }),
        );
        assert!(!refused);
        assert_eq!(
            (now(&store).attack[0], now(&store).defense),
            (10045, DEFENSE)
        );
        // Refused: a number no registration has, no number, a team that cannot be, and the
        // teams of another registration.
        let (other, _) = store.register(ATTACK, DEFENSE, &Source::default()).unwrap();
        turned_down(
            &store,
            "edit_formation",
            json!({ "id": 99, "wins": 1, "battles": 1 }),
        );
        turned_down(&store, "edit_formation", json!({ "battles": 1, "wins": 1 }));
        turned_down(
            &store,
            "edit_formation",
            json!({ "id": id, "attack": ["アコ"] }),
        );
        turned_down(
            &store,
            "edit_formation",
            json!({ "id": id, "battles": 1, "wins": 2 }),
        );
        let taken = turned_down(
            &store,
            "edit_formation",
            json!({ "id": id, "attack": attack() }),
        );
        assert!(
            taken["error"]
                .as_str()
                .unwrap()
                .contains(&other.to_string()),
            "{taken}"
        );
    }

    #[test]
    fn a_registration_is_taken_away_by_its_number() {
        let store = store();
        let (id, _) = store.register(ATTACK, DEFENSE, &Source::default()).unwrap();
        let (kept, _) = store
            .register(
                [10045, 10037, 10059, 10006, 20008, 20016],
                DEFENSE,
                &Source::default(),
            )
            .unwrap();
        let (refused, done) = call(&store, "delete_formation", json!({ "id": id }));
        assert!(!refused, "{done}");
        assert_eq!(done["deleted"]["id"], id);
        assert_eq!(ids(&done["deleted"]["attack"]), ATTACK, "what it was");
        let left: Vec<i64> = store
            .registered()
            .unwrap()
            .iter()
            .map(|one| one.id)
            .collect();
        assert_eq!(left, [kept]);
        turned_down(&store, "delete_formation", json!({ "id": id }));
        turned_down(&store, "delete_formation", json!({}));
    }

    #[test]
    fn the_lines_are_answered_one_by_one() {
        let (store, roster) = (store(), roster());
        let lines = [
            json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }).to_string(),
            json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }).to_string(),
            String::new(),
            json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" }).to_string(),
        ];
        let mut answers = Vec::new();
        serve(&store, &roster, lines.join("\r\n").as_bytes(), &mut answers).unwrap();
        let answers = String::from_utf8(answers).unwrap();
        let ids: Vec<Value> = (answers.lines())
            .map(|line| serde_json::from_str::<Value>(line).unwrap()["id"].clone())
            .collect();
        assert_eq!(
            ids,
            [json!(1), json!(2)],
            "a line for each request, none for the rest"
        );
        assert!(answers.ends_with('\n'));
    }
}
