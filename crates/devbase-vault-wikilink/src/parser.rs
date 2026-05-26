// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094

use crate::WikiLink;

pub fn parse_link(inner: &str, start: usize, end: usize) -> WikiLink {
    let (left, display) = if let Some(pipe_pos) = inner.find('|') {
        (inner[..pipe_pos].trim(), Some(inner[pipe_pos + 1..].trim().to_string()))
    } else {
        (inner.trim(), None)
    };

    let (target, anchor) = if let Some(hash_pos) = left.find('#') {
        (
            left[..hash_pos].trim().to_string(),
            Some(left[hash_pos + 1..].trim().to_string()),
        )
    } else {
        (left.to_string(), None)
    };

    WikiLink {
        target,
        display,
        anchor,
        start,
        end,
    }
}
