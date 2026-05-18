use crate::db::SchemaNode;

#[derive(Debug, Clone, PartialEq)]
pub enum TriggerKind {
    /// Triggered by a SQL keyword (FROM, JOIN, etc).
    Table,
    /// Triggered by a `.` after an identifier the cache resolves.
    Dot,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CompletionContext {
    pub kind: TriggerKind,
    pub anchor_path: Vec<String>,
    pub partial: String,
    pub replace_row: usize,
    /// Column where the full trailing token (including dots) starts.
    pub replace_col_start: usize,
    /// Column equal to the cursor column.
    pub replace_col_end: usize,
}

#[derive(Debug, Clone)]
pub struct CompletionState {
    pub suggestions: Vec<String>,
    pub selected: usize,
    pub ctx: CompletionContext,
}

/// Returns true if `c` is a valid identifier character (excluding `.`).
fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Returns true if `c` is a valid token character (identifier or dot).
fn is_token_char(c: char) -> bool {
    is_ident_char(c) || c == '.'
}

/// Detect completion context given the editor buffer and cursor position.
pub fn detect_context(buffer: &str, cursor: (usize, usize)) -> Option<CompletionContext> {
    let (row, col) = cursor;
    let line = buffer.lines().nth(row).unwrap_or("");
    // Slice line up to cursor; cursor col is char-indexed.
    let prefix: String = line.chars().take(col).collect();
    let prefix_chars: Vec<char> = prefix.chars().collect();

    // Walk back from cursor to find start of trailing token.
    let mut token_start = prefix_chars.len();
    while token_start > 0 && is_token_char(prefix_chars[token_start - 1]) {
        token_start -= 1;
    }
    let token: String = prefix_chars[token_start..].iter().collect();

    // Split token on '.' — last segment is partial, preceding are anchor_path.
    let mut segments: Vec<&str> = token.split('.').collect();
    let partial = segments.pop().unwrap_or("").to_string();
    let anchor_path: Vec<String> = segments.iter().map(|s| (*s).to_string()).collect();

    // If anchor non-empty → Dot trigger (always active).
    if !anchor_path.is_empty() {
        return Some(CompletionContext {
            kind: TriggerKind::Dot,
            anchor_path,
            partial,
            replace_row: row,
            replace_col_start: token_start,
            replace_col_end: col,
        });
    }

    // Otherwise check if the chars before the token match a trigger keyword.
    let before: String = prefix_chars[..token_start].iter().collect();
    let before_trimmed = before.trim_end();
    // Must have at least one whitespace char between keyword and token, OR
    // token at start of line with keyword preceding it on a previous line.
    // Simpler: require `before` ends with whitespace (after trim, this means
    // before != before_trimmed) OR before is empty (no keyword either).
    if before.len() == before_trimmed.len() {
        // No whitespace between previous content and token — not a trigger.
        return None;
    }

    if matches_trigger_keyword(before_trimmed) {
        Some(CompletionContext {
            kind: TriggerKind::Table,
            anchor_path,
            partial,
            replace_row: row,
            replace_col_start: token_start,
            replace_col_end: col,
        })
    } else {
        None
    }
}

/// Whether `s` ends with one of the trigger keyword patterns, preceded by
/// start-of-string or whitespace.
fn matches_trigger_keyword(s: &str) -> bool {
    // Patterns ordered longest-first so e.g. "LEFT OUTER JOIN" is checked
    // before "JOIN".
    const PATTERNS: &[&str] = &[
        "INNER JOIN",
        "LEFT OUTER JOIN",
        "RIGHT OUTER JOIN",
        "FULL OUTER JOIN",
        "LEFT JOIN",
        "RIGHT JOIN",
        "FULL JOIN",
        "CROSS JOIN",
        "INSERT INTO",
        "CREATE TABLE",
        "DROP TABLE",
        "FROM",
        "JOIN",
        "UPDATE",
        "TRUNCATE",
    ];

    let upper = s.to_ascii_uppercase();
    // Collapse internal whitespace runs so "INSERT  INTO" matches "INSERT INTO".
    let collapsed = collapse_whitespace(&upper);

    for pat in PATTERNS {
        if collapsed.ends_with(pat) {
            // Verify the keyword is at start or preceded by a non-identifier char.
            let key_start = collapsed.len() - pat.len();
            if key_start == 0 {
                return true;
            }
            let prev = collapsed.as_bytes()[key_start - 1] as char;
            if !is_ident_char(prev) {
                return true;
            }
        }
    }
    false
}

fn collapse_whitespace(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out
}

/// Compute completion suggestions. Each returned string is the full text to
/// substitute into the editor (replacing the trailing token, including any
/// dots).
pub fn compute_suggestions(schema: &[SchemaNode], ctx: &CompletionContext) -> Vec<String> {
    // Try to resolve anchor_path against the schema tree.
    let resolved = resolve_path(schema, &ctx.anchor_path);

    if let Some(children) = resolved {
        let effective = effective_children(children);
        let mut scored: Vec<(i32, String)> = effective
            .iter()
            .filter_map(|n| {
                let score = if ctx.partial.is_empty() {
                    0
                } else {
                    fuzzy_match(&ctx.partial, &n.label)?
                };
                let prefix = if ctx.anchor_path.is_empty() {
                    String::new()
                } else {
                    format!("{}.", ctx.anchor_path.join("."))
                };
                Some((score, format!("{prefix}{}", n.label)))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        if !scored.is_empty() {
            return scored.into_iter().map(|(_, s)| s).collect();
        }
        // Resolved but nothing matched the partial — for Dot trigger we're done;
        // for Table trigger, fall through to flat fuzzy across all tables.
        if ctx.kind == TriggerKind::Dot {
            return Vec::new();
        }
    } else if ctx.kind == TriggerKind::Dot {
        return Vec::new();
    }

    // Flat fallback: enumerate all table-depth paths and fuzzy-rank.
    let mut paths: Vec<Vec<String>> = Vec::new();
    for node in schema {
        collect_table_paths(node, &mut Vec::new(), &mut paths);
    }

    let needle = ctx.partial.as_str();
    let mut scored: Vec<(i32, String)> = paths
        .into_iter()
        .filter_map(|p| {
            let joined = p.join(".");
            let score = if needle.is_empty() {
                0
            } else {
                fuzzy_match(needle, &joined)?
            };
            Some((score, joined))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    scored.truncate(20);
    scored.into_iter().map(|(_, s)| s).collect()
}

/// Walk children to find the children of the node addressed by `path`.
/// Returns `None` if any segment of `path` doesn't match (case-insensitive).
/// Empty path returns the root list as children. Group nodes (Tables/Views)
/// are transparently flattened during traversal so users never have to type
/// them as a path segment.
fn resolve_path<'a>(roots: &'a [SchemaNode], path: &[String]) -> Option<&'a [SchemaNode]> {
    let mut current: &[SchemaNode] = roots;
    for segment in path {
        let effective = effective_children(current);
        let next = effective
            .into_iter()
            .find(|n| n.label.eq_ignore_ascii_case(segment))?;
        current = &next.children;
    }
    Some(current)
}

/// Flatten group nodes (Tables/Views) one level, exposing their children in
/// place of the group itself. Non-group nodes pass through unchanged.
fn effective_children(children: &[SchemaNode]) -> Vec<&SchemaNode> {
    let mut out = Vec::new();
    for n in children {
        if is_group_label(&n.label) {
            for c in &n.children {
                out.push(c);
            }
        } else {
            out.push(n);
        }
    }
    out
}

fn is_group_label(s: &str) -> bool {
    s.eq_ignore_ascii_case("Tables") || s.eq_ignore_ascii_case("Views")
}

/// Collect all root-to-table paths from `node`. A "table" is a node whose
/// children are all leaves (e.g. columns) or which has no children at all.
/// Skips well-known group-label segments ("Tables", "Views") in the output path.
fn collect_table_paths(node: &SchemaNode, stack: &mut Vec<String>, out: &mut Vec<Vec<String>>) {
    let is_group = node.label.eq_ignore_ascii_case("Tables")
        || node.label.eq_ignore_ascii_case("Views");
    if !is_group {
        stack.push(node.label.clone());
    }

    let children_are_leaves = !node.children.is_empty()
        && node.children.iter().all(|c| c.children.is_empty());

    if node.children.is_empty() || children_are_leaves {
        // Treat as a table (or leaf), but only if we actually have a path.
        if !stack.is_empty() && !is_group {
            out.push(stack.clone());
        }
    } else {
        for child in &node.children {
            collect_table_paths(child, stack, out);
        }
    }

    if !is_group {
        stack.pop();
    }
}

/// Subsequence fuzzy match. Returns a score (higher = better) if every char of
/// `needle` appears in order in `hay` (case-insensitive). Bonuses for
/// consecutive matches and prefix matches.
pub fn fuzzy_match(needle: &str, hay: &str) -> Option<i32> {
    if needle.is_empty() {
        return Some(0);
    }
    let needle_lc = needle.to_ascii_lowercase();
    let hay_lc = hay.to_ascii_lowercase();
    let needle_bytes = needle_lc.as_bytes();
    let hay_bytes = hay_lc.as_bytes();

    let mut ni = 0;
    let mut score: i32 = 0;
    let mut last_matched: Option<usize> = None;
    let mut consecutive = 0;

    for (hi, &h) in hay_bytes.iter().enumerate() {
        if ni < needle_bytes.len() && h == needle_bytes[ni] {
            let bonus = if hi == 0 { 10 } else { 0 };
            let consec_bonus = if hi > 0 && last_matched == Some(hi - 1) {
                consecutive += 1;
                3 * consecutive
            } else {
                consecutive = 0;
                0
            };
            score += 5 + bonus + consec_bonus;
            last_matched = Some(hi);
            ni += 1;
        }
    }

    if ni == needle_bytes.len() {
        // Penalize length (prefer shorter hays at equal raw score).
        score -= (hay_bytes.len() as i32) / 4;
        Some(score)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx_at(buffer: &str) -> Option<CompletionContext> {
        // Cursor = end of last line.
        let lines: Vec<&str> = buffer.split('\n').collect();
        let last_row = lines.len() - 1;
        let last_col = lines[last_row].chars().count();
        detect_context(buffer, (last_row, last_col))
    }

    #[test]
    fn detect_from_empty_partial() {
        let c = ctx_at("SELECT * FROM ").unwrap();
        assert_eq!(c.kind, TriggerKind::Table);
        assert!(c.anchor_path.is_empty());
        assert_eq!(c.partial, "");
    }

    #[test]
    fn detect_from_with_partial() {
        let c = ctx_at("SELECT * FROM pub").unwrap();
        assert_eq!(c.kind, TriggerKind::Table);
        assert_eq!(c.partial, "pub");
        assert!(c.anchor_path.is_empty());
        assert_eq!(c.replace_col_start, "SELECT * FROM ".len());
        assert_eq!(c.replace_col_end, "SELECT * FROM pub".len());
    }

    #[test]
    fn detect_dot_anchor() {
        let c = ctx_at("SELECT * FROM mydb.pu").unwrap();
        assert_eq!(c.kind, TriggerKind::Dot);
        assert_eq!(c.anchor_path, vec!["mydb".to_string()]);
        assert_eq!(c.partial, "pu");
        assert_eq!(c.replace_col_start, "SELECT * FROM ".len());
    }

    #[test]
    fn detect_dot_two_segments() {
        let c = ctx_at("FROM a.b.").unwrap();
        assert_eq!(c.kind, TriggerKind::Dot);
        assert_eq!(c.anchor_path, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(c.partial, "");
    }

    #[test]
    fn detect_lowercase_from() {
        assert!(ctx_at("select * from ").is_some());
    }

    #[test]
    fn detect_mixed_case() {
        assert!(ctx_at("Select * From t").is_some());
    }

    #[test]
    fn detect_left_join() {
        assert!(ctx_at("SELECT * FROM a LEFT JOIN ").is_some());
    }

    #[test]
    fn detect_left_outer_join() {
        assert!(ctx_at("SELECT * FROM a LEFT OUTER JOIN ").is_some());
    }

    #[test]
    fn detect_cross_join() {
        assert!(ctx_at("SELECT * FROM a CROSS JOIN ").is_some());
    }

    #[test]
    fn detect_inner_join() {
        assert!(ctx_at("SELECT * FROM a INNER JOIN ").is_some());
    }

    #[test]
    fn detect_plain_join() {
        assert!(ctx_at("SELECT * FROM a JOIN ").is_some());
    }

    #[test]
    fn detect_insert_into() {
        assert!(ctx_at("INSERT INTO ").is_some());
    }

    #[test]
    fn detect_insert_into_double_space() {
        assert!(ctx_at("insert  into ").is_some());
    }

    #[test]
    fn detect_create_table() {
        assert!(ctx_at("CREATE TABLE ").is_some());
    }

    #[test]
    fn detect_drop_table() {
        assert!(ctx_at("DROP TABLE ").is_some());
    }

    #[test]
    fn detect_truncate() {
        assert!(ctx_at("TRUNCATE ").is_some());
    }

    #[test]
    fn detect_update() {
        assert!(ctx_at("UPDATE ").is_some());
    }

    #[test]
    fn detect_where_no_trigger() {
        assert!(ctx_at("SELECT * FROM t WHERE x = ").is_none());
    }

    #[test]
    fn detect_no_trigger_inside_alias() {
        // After "FROM foo bar" — bar is an alias, not a fresh trigger.
        assert!(ctx_at("SELECT * FROM foo bar").is_none());
    }

    #[test]
    fn detect_mid_line_uses_only_left_of_cursor() {
        // buffer has trailing text after cursor; only left side is considered.
        let buf = "SELECT * FROM pu WHERE x = 1";
        let cursor = (0, "SELECT * FROM pu".len());
        let c = detect_context(buf, cursor).unwrap();
        assert_eq!(c.partial, "pu");
        assert_eq!(c.kind, TriggerKind::Table);
    }

    #[test]
    fn detect_no_trigger_when_keyword_glued() {
        // "FROMx" should not trigger — keyword must end at whitespace.
        assert!(ctx_at("FROMx").is_none());
    }

    // ---- compute_suggestions ----

    fn fixture() -> Vec<SchemaNode> {
        vec![
            SchemaNode::group("catA", vec![
                SchemaNode::group("scA", vec![
                    SchemaNode::group("Tables", vec![
                        SchemaNode::group("users", vec![SchemaNode::leaf("id")]),
                        SchemaNode::group("flushed_records", vec![SchemaNode::leaf("id")]),
                    ]),
                ]),
                SchemaNode::group("scB", vec![
                    SchemaNode::group("Tables", vec![
                        SchemaNode::group("orders", vec![SchemaNode::leaf("id")]),
                    ]),
                ]),
            ]),
            SchemaNode::group("catB", vec![
                SchemaNode::group("scX", vec![
                    SchemaNode::group("Tables", vec![
                        SchemaNode::group("items", vec![SchemaNode::leaf("id")]),
                    ]),
                ]),
            ]),
        ]
    }

    fn ctx(anchor: &[&str], partial: &str, kind: TriggerKind) -> CompletionContext {
        CompletionContext {
            kind,
            anchor_path: anchor.iter().map(|s| s.to_string()).collect(),
            partial: partial.to_string(),
            replace_row: 0,
            replace_col_start: 0,
            replace_col_end: 0,
        }
    }

    #[test]
    fn suggest_top_level() {
        let s = compute_suggestions(&fixture(), &ctx(&[], "", TriggerKind::Table));
        assert!(s.iter().any(|x| x == "catA"));
        assert!(s.iter().any(|x| x == "catB"));
    }

    #[test]
    fn suggest_filtered_by_partial() {
        let s = compute_suggestions(&fixture(), &ctx(&[], "catA", TriggerKind::Table));
        // catA should rank first
        assert_eq!(s[0], "catA");
    }

    #[test]
    fn suggest_under_catalog() {
        let s = compute_suggestions(&fixture(), &ctx(&["catA"], "", TriggerKind::Dot));
        assert!(s.contains(&"catA.scA".to_string()));
        assert!(s.contains(&"catA.scB".to_string()));
    }

    #[test]
    fn suggest_under_schema_skips_tables_group() {
        // catA.scA.* should expose tables directly, not the "Tables" group node.
        let s = compute_suggestions(&fixture(), &ctx(&["catA", "scA"], "", TriggerKind::Dot));
        assert!(
            s.iter().all(|x| !x.contains("Tables")),
            "Tables group leaked into suggestions: {s:?}"
        );
        assert!(s.iter().any(|x| x == "catA.scA.users"));
        assert!(s.iter().any(|x| x == "catA.scA.flushed_records"));
    }

    #[test]
    fn suggest_top_level_partial_no_catalog_match_falls_back_flat() {
        // Typing "FROM users" when no catalog is named "users" should still
        // find the table via flat fuzzy fallback.
        let s = compute_suggestions(&fixture(), &ctx(&[], "users", TriggerKind::Table));
        assert!(
            s.iter().any(|x| x.ends_with(".users")),
            "expected flat fallback to surface a users table, got {s:?}"
        );
    }

    #[test]
    fn suggest_unresolved_anchor_dot_returns_empty() {
        let s = compute_suggestions(&fixture(), &ctx(&["nope"], "x", TriggerKind::Dot));
        assert!(s.is_empty());
    }

    #[test]
    fn suggest_unresolved_anchor_table_falls_back_flat() {
        let s = compute_suggestions(&fixture(), &ctx(&["bogus"], "users", TriggerKind::Table));
        // Flat fallback returns full table paths matching "users"
        assert!(s.iter().any(|x| x.contains("users")));
    }

    #[test]
    fn fuzzy_subseq_matches() {
        assert!(fuzzy_match("usr", "users").is_some());
    }

    #[test]
    fn fuzzy_users_ranks_above_flushed_records() {
        let a = fuzzy_match("usr", "users").unwrap();
        let b = fuzzy_match("usr", "flushed_records").unwrap();
        assert!(a > b, "expected users ({a}) > flushed_records ({b})");
    }

    #[test]
    fn fuzzy_non_match_returns_none() {
        assert!(fuzzy_match("zzz", "users").is_none());
    }
}
