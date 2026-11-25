//! An opaque identity for one appearance of an application-owned menu.
//!
//! A menu's position and its items look the same every time it is opened, so
//! neither can tell two openings apart. What does is the identity of the menu
//! element itself: platforms that expose a cross-process element token answer
//! with one that survives the process that read it, and platforms that do not
//! fall back to a digest of the menu's whole structure, which at least notices
//! any change the caller could observe.

use crate::AccessibilityNode;

const INSTANCE_PREFIX: &str = "mnu1:";
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const MAX_HASHED_NODES: usize = 4_096;
const MAX_HASHED_DEPTH: usize = 3;

/// What a menu-instance id was derived from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstanceBasis {
    RemoteToken,
    Structure,
}

impl InstanceBasis {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::RemoteToken => "remote_token",
            Self::Structure => "structure",
        }
    }
}

/// One reading of a menu surface, reduced to what identifies that appearance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MenuInstance {
    id: String,
    basis: InstanceBasis,
}

impl MenuInstance {
    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn basis(&self) -> InstanceBasis {
        self.basis
    }
}

/// Reads the instance of the menu `tree` describes.
///
/// A node carrying a platform element token answers with that token, hashed so
/// the id stays opaque and so two readings of one appearance agree; anything
/// else answers with a digest of the menu's structure.
pub(crate) fn of_tree(tree: &AccessibilityNode) -> MenuInstance {
    if let Some(token) = tree
        .identity
        .retained_object
        .as_deref()
        .filter(|token| !token.is_empty())
    {
        return MenuInstance {
            id: instance_id("token", token),
            basis: InstanceBasis::RemoteToken,
        };
    }
    let mut canonical = String::new();
    append_structure(tree, &mut canonical, 0, &mut 0_usize);
    MenuInstance {
        id: instance_id("structure", &canonical),
        basis: InstanceBasis::Structure,
    }
}

fn instance_id(kind: &str, value: &str) -> String {
    let mut bytes = Vec::with_capacity(kind.len() + value.len() + 1);
    bytes.extend_from_slice(kind.as_bytes());
    bytes.push(b'\n');
    bytes.extend_from_slice(value.as_bytes());
    format!("{INSTANCE_PREFIX}{:016x}", fnv1a(&bytes))
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

fn field(value: Option<&str>) -> &str {
    value.unwrap_or("")
}

/// Appends the labels and roles that make up `node`'s structure, three levels
/// deep.
///
/// The digest deliberately leaves out geometry, child counts and truncation
/// markers: two reads of one menu with different `--include-bounds`, depth or
/// time budgets differ in exactly those fields, and a digest that noticed the
/// difference would report no provenance for a menu nobody disputed. Equality
/// only ever *refuses* a claim, so the fields kept are the ones a caller can
/// see and a menu cannot fake.
fn append_structure(node: &AccessibilityNode, out: &mut String, depth: usize, visited: &mut usize) {
    *visited += 1;
    if *visited > MAX_HASHED_NODES {
        return;
    }
    out.push_str(node.role.as_str());
    out.push('\u{1f}');
    out.push_str(field(node.identity.name.as_deref()));
    out.push('\u{1f}');
    out.push_str(field(node.identity.value.as_deref()));
    out.push('\u{1f}');
    out.push_str(field(node.identity.description.as_deref()));
    out.push('\u{1f}');
    out.push_str(&node.presentation.states.join(","));
    out.push('\u{1e}');
    if depth >= MAX_HASHED_DEPTH {
        return;
    }
    for child in &node.children {
        append_structure(child, out, depth + 1, visited);
    }
}
