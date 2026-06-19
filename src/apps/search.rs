use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config as MatcherConfig, Matcher, Utf32Str};

pub struct FuzzySearch {
    matcher: Matcher,
    utf32_buf: Vec<char>,
}

impl Default for FuzzySearch {
    fn default() -> Self {
        Self {
            matcher: Matcher::new(MatcherConfig::DEFAULT),
            utf32_buf: Vec::new(),
        }
    }
}

impl FuzzySearch {
    pub fn rank(&mut self, query: &str, labels: &[&str], max: usize) -> Vec<(usize, u32)> {
        let q = query.trim();
        if q.is_empty() {
            return (0..labels.len().min(max)).map(|i| (i, 0)).collect();
        }
        let pattern = Pattern::parse(q, CaseMatching::Ignore, Normalization::Smart);
        let mut scored = Vec::new();
        for (idx, label) in labels.iter().enumerate() {
            let haystack = Utf32Str::new(label, &mut self.utf32_buf);
            if let Some(score) = pattern.score(haystack, &mut self.matcher) {
                if score > 0 {
                    scored.push((idx, score));
                }
            }
        }
        scored.sort_by_key(|b| std::cmp::Reverse(b.1));
        scored.truncate(max);
        scored
    }
}
