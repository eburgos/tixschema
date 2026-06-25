use super::*;

#[test]
fn test_feature_detection() {
    // Test that we can detect features at compile time
    let enabled = Features::enabled_features();
    log::debug!("Enabled features: {enabled:?}");

    // In default configuration, all features should be enabled
    #[cfg(all(
        feature = "serde",
        feature = "zod",
        feature = "jsonschema",
        feature = "object_id",
        feature = "typescript"
    ))]
    {
        assert!(Features::has_serde());
        assert!(Features::has_zod());
        assert!(Features::has_jsonschema());
        assert!(Features::has_object_id());
        assert!(Features::has_typescript());
    }
}
