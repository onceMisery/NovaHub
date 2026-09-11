#![forbid(unsafe_code)]

use std::cmp::Ordering;

use novahub_core_domain::CommandDescriptor;
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};

const MAX_RESULTS: usize = 50;

/// Bounded result batch returned to the host renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SearchResult {
    commands: Vec<CommandDescriptor>,
}

impl SearchResult {
    #[must_use]
    pub fn new(mut commands: Vec<CommandDescriptor>) -> Self {
        commands.truncate(MAX_RESULTS);
        Self { commands }
    }

    #[must_use]
    pub fn commands(&self) -> &[CommandDescriptor] {
        &self.commands
    }
}

/// Generation gate that prevents stale providers from repainting the shell.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SearchCoordinator {
    generation: u64,
}

/// Reusable fuzzy matcher for the bounded command snapshot.
///
/// `Matcher` owns an internal scratch slab. Keeping it alive across keystrokes
/// avoids reallocating that slab on every query while keeping all matching on
/// the caller's worker/UI boundary.
#[derive(Debug)]
pub struct CommandMatcher {
    matcher: Matcher,
    pattern: Pattern,
    text_buffer: Vec<char>,
}

impl Default for CommandMatcher {
    fn default() -> Self {
        let mut config = Config::DEFAULT;
        config.prefer_prefix = true;
        Self {
            matcher: Matcher::new(config),
            pattern: Pattern::default(),
            text_buffer: Vec::new(),
        }
    }
}

impl CommandMatcher {
    /// Matches and ranks commands, returning at most [`MAX_RESULTS`] entries.
    #[must_use]
    pub fn match_commands(
        &mut self,
        query: impl AsRef<str>,
        commands: &[CommandDescriptor],
    ) -> Vec<CommandDescriptor> {
        let query = query.as_ref().trim();
        if query.is_empty() {
            return commands.iter().take(MAX_RESULTS).cloned().collect();
        }

        // Parse once per query and reuse the allocation held by Pattern.
        self.pattern
            .reparse(query, CaseMatching::Ignore, Normalization::Smart);

        let mut ranked = commands
            .iter()
            .enumerate()
            .filter_map(|(index, command)| {
                let score = self.best_field_score(command);
                score.map(|score| RankedCommand {
                    command: command.clone(),
                    score,
                    index,
                })
            })
            .collect::<Vec<_>>();

        ranked.sort_unstable_by(RankedCommand::compare);
        ranked
            .into_iter()
            .take(MAX_RESULTS)
            .map(|ranked| ranked.command)
            .collect()
    }

    fn best_field_score(&mut self, command: &CommandDescriptor) -> Option<u32> {
        [
            command.title.as_str(),
            command.subtitle.as_str(),
            command.id.as_str(),
        ]
        .into_iter()
        .filter_map(|field| {
            self.pattern.score(
                Utf32Str::new(field, &mut self.text_buffer),
                &mut self.matcher,
            )
        })
        .max()
    }
}

#[derive(Debug)]
struct RankedCommand {
    command: CommandDescriptor,
    score: u32,
    index: usize,
}

impl RankedCommand {
    fn compare(left: &Self, right: &Self) -> Ordering {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.index.cmp(&right.index))
    }
}

impl SearchCoordinator {
    #[must_use]
    pub const fn new() -> Self {
        Self { generation: 0 }
    }

    pub fn begin(&mut self, _query: impl AsRef<str>) -> u64 {
        self.generation = self.generation.saturating_add(1);
        self.generation
    }

    #[must_use]
    pub const fn max_results(&self) -> usize {
        MAX_RESULTS
    }

    #[must_use]
    pub fn accept(&self, generation: u64, result: SearchResult) -> Option<SearchResult> {
        (generation == self.generation).then_some(result)
    }
}

#[must_use]
/// Convenience wrapper for one-shot callers. Interactive hosts should retain
/// a [`CommandMatcher`] so its scratch allocation is reused across keystrokes.
pub fn match_commands(query: &str, commands: &[CommandDescriptor]) -> Vec<CommandDescriptor> {
    CommandMatcher::default().match_commands(query, commands)
}

#[cfg(test)]
mod tests {
    use super::{CommandMatcher, SearchCoordinator, SearchResult, match_commands};
    use novahub_core_domain::{CommandDescriptor, CommandId};

    fn result(id: &str) -> SearchResult {
        SearchResult::new(vec![CommandDescriptor {
            id: CommandId::new(id),
            title: id.into(),
            subtitle: String::new(),
        }])
    }

    #[test]
    fn stale_query_results_are_discarded_after_new_generation() {
        let mut coordinator = SearchCoordinator::new();
        let first = coordinator.begin("calc");
        let second = coordinator.begin("clip");

        assert!(coordinator.accept(first, result("calculator")).is_none());
        assert_eq!(
            coordinator
                .accept(second, result("clipboard"))
                .expect("latest generation is accepted")
                .commands()[0]
                .id
                .as_str(),
            "clipboard"
        );
    }

    #[test]
    fn empty_query_starts_at_generation_one_and_is_bounded() {
        let mut coordinator = SearchCoordinator::new();
        let generation = coordinator.begin(" ");
        assert_eq!(generation, 1);
        assert_eq!(coordinator.max_results(), 50);
    }

    #[test]
    fn command_subtitles_participate_in_matching() {
        let commands = vec![CommandDescriptor {
            id: CommandId::new("apps.launch"),
            title: "Applications".into(),
            subtitle: "Launch an application".into(),
        }];
        assert_eq!(match_commands("launch", &commands), commands);
    }

    #[test]
    fn fuzzy_matching_ranks_contiguous_prefixes_before_sparse_matches() {
        let commands = vec![
            CommandDescriptor {
                id: CommandId::new("clipboard.history"),
                title: "Clipboard History".into(),
                subtitle: "Review copied text".into(),
            },
            CommandDescriptor {
                id: CommandId::new("system.command"),
                title: "System Command".into(),
                subtitle: "Run a shell command".into(),
            },
        ];
        let mut matcher = CommandMatcher::default();

        let results = matcher.match_commands("clip", &commands);

        assert_eq!(results[0].id.as_str(), "clipboard.history");
    }

    #[test]
    fn matcher_reuses_the_same_instance_for_multiple_queries() {
        let commands = vec![CommandDescriptor {
            id: CommandId::new("apps.launch"),
            title: "Applications".into(),
            subtitle: "Launch an application".into(),
        }];
        let mut matcher = CommandMatcher::default();

        assert_eq!(matcher.match_commands("app", &commands), commands);
        assert_eq!(matcher.match_commands("launch", &commands), commands);
    }
}
