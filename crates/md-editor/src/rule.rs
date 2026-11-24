//! Ordered, static rule table for intent dispatch.

use std::fmt;

use crate::intent::MoveDirection;
use crate::{Editor, Intent};

/// Stable rule identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RuleId(u16);

impl RuleId {
    /// Move cursor left by one grapheme/atom.
    pub const MOVE_LEFT: Self = Self(0x0101);
    /// Move cursor right by one grapheme/atom.
    pub const MOVE_RIGHT: Self = Self(0x0102);
    /// Move to document start.
    pub const MOVE_DOC_START: Self = Self(0x0103);
    /// Move to document end.
    pub const MOVE_DOC_END: Self = Self(0x0104);
    /// Insert literal text.
    pub const INSERT_TEXT: Self = Self(0x0201);
    /// Delete backward by grapheme/atom.
    pub const DELETE_BACKWARD: Self = Self(0x0202);
    /// Split the current block at the caret.
    pub const ENTER: Self = Self(0x0203);

    /// Returns the numeric id.
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl fmt::Display for RuleId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "R{:04X}", self.0)
    }
}

/// Context available to rule guards.
pub struct Ctx<'a> {
    /// Current editor state.
    pub editor: &'a Editor,
    /// Intent being dispatched.
    pub intent: &'a Intent,
}

/// One ordered rule entry.
pub struct Rule {
    /// Stable rule id.
    pub id: RuleId,
    /// Guard evaluated in table order.
    pub guard: fn(&Ctx<'_>) -> bool,
}

/// Matches the first rule whose guard accepts the intent.
#[must_use]
pub fn match_rule(editor: &Editor, intent: &Intent) -> Option<RuleId> {
    let ctx = Ctx { editor, intent };
    MOVE_RULES
        .iter()
        .find(|rule| (rule.guard)(&ctx))
        .map(|rule| rule.id)
}

fn move_left(ctx: &Ctx<'_>) -> bool {
    matches!(
        ctx.intent,
        Intent::Move {
            direction: MoveDirection::Left,
            ..
        }
    )
}

fn move_right(ctx: &Ctx<'_>) -> bool {
    matches!(
        ctx.intent,
        Intent::Move {
            direction: MoveDirection::Right,
            ..
        }
    )
}

fn move_doc_start(ctx: &Ctx<'_>) -> bool {
    matches!(
        ctx.intent,
        Intent::Move {
            direction: MoveDirection::DocStart,
            ..
        }
    )
}

fn move_doc_end(ctx: &Ctx<'_>) -> bool {
    matches!(
        ctx.intent,
        Intent::Move {
            direction: MoveDirection::DocEnd,
            ..
        }
    )
}

fn insert_text(ctx: &Ctx<'_>) -> bool {
    matches!(ctx.intent, Intent::InsertText(_))
}

fn delete_backward(ctx: &Ctx<'_>) -> bool {
    matches!(ctx.intent, Intent::DeleteBackward)
}

fn enter(ctx: &Ctx<'_>) -> bool {
    matches!(ctx.intent, Intent::InsertNewline)
}

static MOVE_RULES: &[Rule] = &[
    Rule {
        id: RuleId::MOVE_LEFT,
        guard: move_left,
    },
    Rule {
        id: RuleId::MOVE_RIGHT,
        guard: move_right,
    },
    Rule {
        id: RuleId::MOVE_DOC_START,
        guard: move_doc_start,
    },
    Rule {
        id: RuleId::MOVE_DOC_END,
        guard: move_doc_end,
    },
    Rule {
        id: RuleId::INSERT_TEXT,
        guard: insert_text,
    },
    Rule {
        id: RuleId::DELETE_BACKWARD,
        guard: delete_backward,
    },
    Rule {
        id: RuleId::ENTER,
        guard: enter,
    },
];

#[cfg(test)]
mod tests {
    use super::{RuleId, match_rule};
    use crate::intent::MoveDirection;
    use crate::{Editor, Intent};
    use md_document::Document;

    #[test]
    fn rule_ids_have_distinct_values_and_display() {
        let ids = [
            RuleId::MOVE_LEFT,
            RuleId::MOVE_RIGHT,
            RuleId::MOVE_DOC_START,
            RuleId::MOVE_DOC_END,
        ];
        for (index, left) in ids.iter().enumerate() {
            for right in &ids[index + 1..] {
                assert_ne!(left, right);
            }
        }
        assert_eq!(RuleId::MOVE_LEFT.to_string(), "R0101");
    }

    #[test]
    fn move_intents_match_in_order() {
        let editor = Editor::new(Document::new());
        let intent = Intent::Move {
            direction: MoveDirection::Right,
            extend: false,
        };
        assert_eq!(match_rule(&editor, &intent), Some(RuleId::MOVE_RIGHT));
    }

    #[test]
    fn unsupported_intent_has_no_rule() {
        let editor = Editor::new(Document::new());
        assert_eq!(match_rule(&editor, &Intent::Paste("x".to_owned())), None);
    }
}
