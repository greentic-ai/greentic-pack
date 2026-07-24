//! Codec for the per-flow event-topic carried in `PackFlowEntry.tags`.
//!
//! Until a first-class `subscribes_to` field lands upstream in
//! `greentic-types::PackFlowEntry`, a flow's inbound event topics ride in the
//! existing `tags` vector under a reserved prefix. This module is the single
//! place that knows the prefix so producers and consumers cannot drift.

/// Reserved `PackFlowEntry.tags` prefix marking an inbound event topic.
pub const EVENT_TOPIC_TAG_PREFIX: &str = "event-topic:";

/// Encode a topic into its tag form (`"orders.created"` → `"event-topic:orders.created"`).
pub fn event_topic_tag(topic: &str) -> String {
    format!("{EVENT_TOPIC_TAG_PREFIX}{}", topic.trim())
}

/// Merge a flow's author tags with its inbound event topics, encoding each
/// non-blank topic as an `event-topic:` tag. Author tags come first, order
/// preserved; the result is de-duplicated by exact string equality
/// (first-seen order kept), so a repeated topic or an author tag that
/// already equals an encoded topic tag is not duplicated.
pub fn tags_from_topics(author_tags: &[String], topics: &[String]) -> Vec<String> {
    let mut tags = author_tags.to_vec();
    for topic in topics {
        if topic.trim().is_empty() {
            continue;
        }
        tags.push(event_topic_tag(topic));
    }

    let mut seen = std::collections::HashSet::new();
    tags.retain(|tag| seen.insert(tag.clone()));
    tags
}

/// Decode all event topics carried in a flow's tags, in order. Non-topic tags
/// are ignored; blank topics are skipped.
pub fn topics_from_tags(tags: &[String]) -> Vec<String> {
    tags.iter()
        .filter_map(|t| t.strip_prefix(EVENT_TOPIC_TAG_PREFIX))
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_topic_with_prefix() {
        assert_eq!(
            event_topic_tag("orders.created"),
            "event-topic:orders.created"
        );
    }

    #[test]
    fn round_trips_encode_then_decode() {
        let tags = vec![
            event_topic_tag("orders.created"),
            event_topic_tag("orders.shipped"),
        ];
        assert_eq!(
            topics_from_tags(&tags),
            vec!["orders.created", "orders.shipped"]
        );
    }

    #[test]
    fn decode_ignores_non_topic_tags_and_blanks() {
        let tags = vec![
            "billing".to_string(),
            "event-topic:orders.created".to_string(),
            "event-topic:   ".to_string(),
            "event-topic:".to_string(),
        ];
        assert_eq!(topics_from_tags(&tags), vec!["orders.created"]);
    }

    #[test]
    fn tags_from_topics_appends_event_topic_tags_after_author_tags() {
        let author_tags = vec!["billing".to_string()];
        let subs = vec![
            "orders.created".to_string(),
            "  ".to_string(),
            "orders.shipped".to_string(),
        ];

        let merged = tags_from_topics(&author_tags, &subs);

        assert_eq!(
            merged,
            vec![
                "billing".to_string(),
                format!("{EVENT_TOPIC_TAG_PREFIX}orders.created"),
                format!("{EVENT_TOPIC_TAG_PREFIX}orders.shipped"),
            ]
        );
    }

    #[test]
    fn tags_from_topics_skips_blank_and_whitespace_only_topics() {
        let author_tags: Vec<String> = vec![];
        let subs = vec!["".to_string(), "   ".to_string()];

        assert!(tags_from_topics(&author_tags, &subs).is_empty());
    }

    #[test]
    fn tags_from_topics_deduplicates_repeated_topics() {
        let author_tags: Vec<String> = vec![];
        let subs = vec!["orders.created".to_string(), "orders.created".to_string()];

        assert_eq!(
            tags_from_topics(&author_tags, &subs),
            vec![event_topic_tag("orders.created")]
        );
    }

    #[test]
    fn tags_from_topics_does_not_duplicate_author_tag_equal_to_encoded_topic() {
        let author_tags = vec![event_topic_tag("orders.created")];
        let subs = vec!["orders.created".to_string()];

        assert_eq!(
            tags_from_topics(&author_tags, &subs),
            vec![event_topic_tag("orders.created")]
        );
    }

    #[test]
    fn tags_from_topics_round_trips_topic_that_looks_like_a_tag() {
        // A topic literally equal to "event-topic:x" encodes to a
        // double-prefixed tag, and a single strip_prefix recovers the
        // original (still-prefixed) topic string.
        let author_tags: Vec<String> = vec![];
        let topic = "event-topic:x".to_string();
        let subs = vec![topic.clone()];

        let tags = tags_from_topics(&author_tags, &subs);
        assert_eq!(tags, vec!["event-topic:event-topic:x".to_string()]);
        assert_eq!(topics_from_tags(&tags), vec![topic]);
    }
}
