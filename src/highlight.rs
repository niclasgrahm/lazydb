use ratatui::style::{Color, Modifier, Style};

const KEYWORDS: &[&str] = &[
    "ADD", "ALL", "ALTER", "AND", "ANY", "AS", "ASC", "ATTACH", "BEGIN", "BETWEEN", "BIGINT",
    "BOOLEAN", "BY", "CASCADE", "CASE", "CAST", "CHECK", "COLUMN", "COMMIT", "CONSTRAINT",
    "CREATE", "CROSS", "DATABASE", "DATE", "DATETIME", "DECIMAL", "DEFAULT", "DELETE", "DESC",
    "DESCRIBE", "DISTINCT", "DOUBLE", "DROP", "ELSE", "END", "ENGINE", "ENUM", "EXCEPT", "EXISTS",
    "EXPLAIN", "FALSE", "FLOAT", "FOR", "FOREIGN", "FORMAT", "FROM", "FULL", "FUNCTION", "GRANT",
    "GROUP", "HAVING", "IF", "ILIKE", "IN", "INDEX", "INNER", "INSERT", "INT", "INTEGER", "INTERSECT",
    "INTERVAL", "INTO", "IS", "JOIN", "KEY", "LEFT", "LIKE", "LIMIT", "MATERIALIZED", "MERGE",
    "NATURAL", "NOT", "NULL", "OFFSET", "ON", "OR", "ORDER", "OUTER", "OVER", "PARTITION",
    "PRIMARY", "PROCEDURE", "REFERENCES", "REPLACE", "RETURNING", "REVOKE", "RIGHT", "ROLLBACK",
    "ROW", "ROWS", "SCHEMA", "SELECT", "SET", "SHOW", "SMALLINT", "STRING", "TABLE", "TEMP",
    "TEMPORARY", "TEXT", "THEN", "TINYINT", "TO", "TRIGGER", "TRUE", "TRUNCATE", "TYPE", "UINT32",
    "UINT64", "UNION", "UNIQUE", "UPDATE", "USING", "VALUES", "VARCHAR", "VIEW", "VIRTUAL",
    "WHEN", "WHERE", "WINDOW", "WITH",
];

const FUNCTIONS: &[&str] = &[
    "ABS", "AVG", "CEIL", "COALESCE", "CONCAT", "COUNT", "CURRENT_DATE", "CURRENT_TIMESTAMP",
    "DATE_TRUNC", "EXTRACT", "FLOOR", "GREATEST", "IFNULL", "LEAST", "LENGTH", "LOWER", "MAX",
    "MIN", "NOW", "NULLIF", "RANK", "ROUND", "ROW_NUMBER", "SUBSTR", "SUBSTRING", "SUM",
    "TRIM", "UPPER",
];

/// Style definitions
const STYLE_KEYWORD: Style = Style::new().fg(Color::Magenta).add_modifier(Modifier::BOLD);
const STYLE_FUNCTION: Style = Style::new().fg(Color::Blue).add_modifier(Modifier::BOLD);
const STYLE_STRING: Style = Style::new().fg(Color::Green);
const STYLE_NUMBER: Style = Style::new().fg(Color::Cyan);
const STYLE_COMMENT: Style = Style::new().fg(Color::DarkGray);
const STYLE_OPERATOR: Style = Style::new().fg(Color::Yellow);
const STYLE_DEFAULT: Style = Style::new().fg(Color::White);

/// A span of highlighted text within a line.
pub struct HlSpan {
    pub text: String,
    pub style: Style,
}

/// Highlight a single line of SQL, returning styled spans.
pub fn highlight_line(line: &str) -> Vec<HlSpan> {
    let mut spans = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Single-line comment: -- ...
        if i + 1 < len && chars[i] == '-' && chars[i + 1] == '-' {
            let rest: String = chars[i..].iter().collect();
            spans.push(HlSpan { text: rest, style: STYLE_COMMENT });
            break;
        }

        // String literal: '...'
        if chars[i] == '\'' {
            let start = i;
            i += 1;
            while i < len {
                if chars[i] == '\'' {
                    if i + 1 < len && chars[i + 1] == '\'' {
                        i += 2; // escaped quote
                    } else {
                        i += 1;
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            let text: String = chars[start..i].iter().collect();
            spans.push(HlSpan { text, style: STYLE_STRING });
            continue;
        }

        // Number
        if chars[i].is_ascii_digit() || (chars[i] == '.' && i + 1 < len && chars[i + 1].is_ascii_digit()) {
            let start = i;
            while i < len && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            spans.push(HlSpan { text, style: STYLE_NUMBER });
            continue;
        }

        // Word (keyword, function, or identifier)
        if chars[i].is_ascii_alphabetic() || chars[i] == '_' {
            let start = i;
            while i < len && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let upper = word.to_uppercase();

            let style = if KEYWORDS.contains(&upper.as_str()) {
                STYLE_KEYWORD
            } else if FUNCTIONS.contains(&upper.as_str()) {
                STYLE_FUNCTION
            } else {
                STYLE_DEFAULT
            };
            spans.push(HlSpan { text: word, style });
            continue;
        }

        // Operators
        if matches!(chars[i], '=' | '<' | '>' | '!' | '+' | '-' | '*' | '/' | '%') {
            let start = i;
            // Consume multi-char operators like !=, >=, <=, ||, ::
            i += 1;
            if i < len && matches!((chars[start], chars[i]),
                ('!', '=') | ('<', '=') | ('>', '=') | ('|', '|') | (':', ':') | ('-', '>')) {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            spans.push(HlSpan { text, style: STYLE_OPERATOR });
            continue;
        }

        // Whitespace and other characters
        let start = i;
        while i < len
            && !chars[i].is_ascii_alphanumeric()
            && chars[i] != '_'
            && chars[i] != '\''
            && chars[i] != '-'
            && !matches!(chars[i], '=' | '<' | '>' | '!' | '+' | '*' | '/' | '%')
        {
            i += 1;
        }
        if i == start {
            // Single character fallback
            i += 1;
        }
        let text: String = chars[start..i].iter().collect();
        spans.push(HlSpan { text, style: STYLE_DEFAULT });
    }

    spans
}
