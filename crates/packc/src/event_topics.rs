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
}
