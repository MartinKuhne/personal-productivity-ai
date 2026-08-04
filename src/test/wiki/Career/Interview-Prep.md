---
title: "Tech Interview Prep Guide"
summary: "Preparation notes for Senior Software Engineer interviews. Covers system design concepts like the CAP theorem and load balancing. Also includes coding practice problems."
tags: ["career", "interview", "tech"]
header-date: 2023-10-01T13:00:00Z
---

# Interview Preparation

Notes for upcoming technical interviews.

## System Design
Important concepts to review:
- CAP Theorem
- Load Balancing strategies
- Database Sharding vs Replication
- Caching (Redis, Memcached)

## Coding Practice (LeetCode)
Focus on the following topics:
1. **Graphs**: BFS, DFS, Dijkstra's
2. **Dynamic Programming**: Knapsack, Longest Common Subsequence
3. **Trees**: Traversals, Tries

### Sample Problem: Two Sum
```rust
use std::collections::HashMap;

pub fn two_sum(nums: Vec<i32>, target: i32) -> Vec<i32> {
    let mut map = HashMap::new();
    for (i, &num) in nums.iter().enumerate() {
        let complement = target - num;
        if let Some(&prev_index) = map.get(&complement) {
            return vec![prev_index as i32, i as i32];
        }
        map.insert(num, i);
    }
    vec![]
}
```
