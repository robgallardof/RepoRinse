//! Pure, in-memory application of fix plans. Writing to disk belongs to `core::safety`.

use sha2::{Digest, Sha256};

use super::model::FixPlan;

/// SHA-256 hex of the UTF-8 bytes of `text`; used as `FixPlan::precondition_hash`.
pub fn hash(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

/// Applies `plan` to `text`. Fails when the text changed since analysis, or when edits overlap,
/// fall outside the text or split a UTF-8 character, so a stale or broken plan never writes anything.
pub fn apply(text: &str, plan: &FixPlan) -> Result<String, String> {
    if hash(text) != plan.precondition_hash {
        return Err(format!(
            "{} changed since it was analysed; fix skipped",
            plan.target
        ));
    }

    let mut edits: Vec<_> = plan.edits.iter().collect();
    edits.sort_by_key(|e| e.start);

    let mut out = String::with_capacity(text.len());
    let mut cursor = 0usize;
    for edit in edits {
        let end = edit
            .start
            .checked_add(edit.len)
            .ok_or("edit length overflows")?;
        if edit.start < cursor
            || end > text.len()
            || !text.is_char_boundary(edit.start)
            || !text.is_char_boundary(end)
        {
            return Err(format!(
                "invalid or overlapping edit in fix plan for {}",
                plan.target
            ));
        }
        out.push_str(&text[cursor..edit.start]);
        out.push_str(&edit.replacement);
        cursor = end;
    }
    out.push_str(&text[cursor..]);
    Ok(out)
}

/// Applies several plans for the same text one after another. Each plan must have been computed
/// against the output of the previous one (re-analyse between fixes of different rules).
pub fn apply_all<'a>(
    text: &str,
    plans: impl IntoIterator<Item = &'a FixPlan>,
) -> Result<String, String> {
    let mut current = text.to_string();
    for plan in plans {
        current = apply(&current, plan)?;
    }
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::TextEdit;

    fn plan(text: &str, edits: Vec<TextEdit>) -> FixPlan {
        FixPlan {
            rule_id: "t".into(),
            target: "f".into(),
            description: String::new(),
            edits,
            precondition_hash: hash(text),
        }
    }

    #[test]
    fn applies_non_overlapping_edits_in_any_order() {
        let text = "a  \nb \n";
        let p = plan(
            text,
            vec![
                TextEdit {
                    start: 5,
                    len: 1,
                    replacement: String::new(),
                },
                TextEdit {
                    start: 1,
                    len: 2,
                    replacement: String::new(),
                },
            ],
        );
        assert_eq!(apply(text, &p).unwrap(), "a\nb\n");
    }

    #[test]
    fn refuses_stale_text() {
        let p = plan("abc", vec![]);
        assert!(apply("abd", &p).is_err());
    }

    #[test]
    fn refuses_overlap_and_split_chars() {
        let text = "ñandú";
        let overlap = plan(
            text,
            vec![
                TextEdit {
                    start: 0,
                    len: 3,
                    replacement: String::new(),
                },
                TextEdit {
                    start: 2,
                    len: 1,
                    replacement: String::new(),
                },
            ],
        );
        assert!(apply(text, &overlap).is_err());
        let split = plan(
            text,
            vec![TextEdit {
                start: 1,
                len: 1,
                replacement: String::new(),
            }],
        );
        assert!(apply(text, &split).is_err());
    }
}
