// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Counter {
    pub id: u64,
    pub value: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct VersionVector {
    pub counters: Vec<Counter>,
}

impl VersionVector {
    /// Increment the counter for `local_id`, creating it if absent.
    #[allow(dead_code)]
    pub fn update(mut self, local_id: u64) -> Self {
        for c in &mut self.counters {
            if c.id == local_id {
                c.value += 1;
                return self;
            }
        }
        self.counters.push(Counter { id: local_id, value: 1 });
        self
    }

    /// Merge with another vector, taking the maximum value for each id.
    #[allow(dead_code)]
    pub fn merge(mut self, other: &VersionVector) -> Self {
        for o in &other.counters {
            let mut found = false;
            for c in &mut self.counters {
                if c.id == o.id {
                    c.value = c.value.max(o.value);
                    found = true;
                    break;
                }
            }
            if !found {
                self.counters.push(o.clone());
            }
        }
        self
    }

    /// Compare two version vectors.
    ///
    /// - Greater  => self dominates other (all >= and at least one >)
    /// - Less     => other dominates self
    /// - Equal    => identical or concurrent conflict (incomparable)
    #[allow(dead_code)]
    pub fn compare(&self, other: &VersionVector) -> Ordering {
        let mut self_map = std::collections::HashMap::new();
        for c in &self.counters {
            self_map.insert(c.id, c.value);
        }
        let mut other_map = std::collections::HashMap::new();
        for c in &other.counters {
            other_map.insert(c.id, c.value);
        }

        let all_ids: std::collections::HashSet<u64> =
            self_map.keys().chain(other_map.keys()).copied().collect();

        let mut has_greater = false;
        let mut has_less = false;
        for id in all_ids {
            let sv = self_map.get(&id).copied().unwrap_or(0);
            let ov = other_map.get(&id).copied().unwrap_or(0);
            if sv > ov {
                has_greater = true;
            } else if sv < ov {
                has_less = true;
            }
        }

        match (has_greater, has_less) {
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            _ => {
                // Both false => equal; both true => conflict => Equal per spec
                Ordering::Equal
            }
        }
    }
}
