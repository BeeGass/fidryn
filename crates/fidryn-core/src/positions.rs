//! Hohfeldian positions. An unqualified `Right` is rejected.

use crate::value::Term;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PositionKind {
    Duty,
    Claim,
    Liberty,
    NoRight,
    Power,
    Liability,
    Immunity,
    Disability,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "PascalCase")]
pub enum Position {
    Duty {
        bearer: String,
        claimant: String,
        content: Term,
    },
    Claim {
        claimant: String,
        bearer: String,
        content: Term,
    },
    Liberty {
        holder: String,
        against: String,
        content: Term,
    },
    NoRight {
        against: String,
        holder: String,
        content: Term,
    },
    Power {
        holder: String,
        subject: String,
        effect: Term,
    },
    Liability {
        subject: String,
        holder: String,
        effect: Term,
    },
    Immunity {
        holder: String,
        against: String,
        effect: Term,
    },
    Disability {
        against: String,
        holder: String,
        effect: Term,
    },
}

impl Position {
    pub fn correlative(&self) -> Self {
        match self {
            Position::Duty {
                bearer,
                claimant,
                content,
            } => Position::Claim {
                claimant: claimant.clone(),
                bearer: bearer.clone(),
                content: content.clone(),
            },
            Position::Claim {
                claimant,
                bearer,
                content,
            } => Position::Duty {
                bearer: bearer.clone(),
                claimant: claimant.clone(),
                content: content.clone(),
            },
            Position::Liberty {
                holder,
                against,
                content,
            } => Position::NoRight {
                against: against.clone(),
                holder: holder.clone(),
                content: content.clone(),
            },
            Position::NoRight {
                against,
                holder,
                content,
            } => Position::Liberty {
                holder: holder.clone(),
                against: against.clone(),
                content: content.clone(),
            },
            Position::Power {
                holder,
                subject,
                effect,
            } => Position::Liability {
                subject: subject.clone(),
                holder: holder.clone(),
                effect: effect.clone(),
            },
            Position::Liability {
                subject,
                holder,
                effect,
            } => Position::Power {
                holder: holder.clone(),
                subject: subject.clone(),
                effect: effect.clone(),
            },
            Position::Immunity {
                holder,
                against,
                effect,
            } => Position::Disability {
                against: against.clone(),
                holder: holder.clone(),
                effect: effect.clone(),
            },
            Position::Disability {
                against,
                holder,
                effect,
            } => Position::Immunity {
                holder: holder.clone(),
                against: against.clone(),
                effect: effect.clone(),
            },
        }
    }
}
